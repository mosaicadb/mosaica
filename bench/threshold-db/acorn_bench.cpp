// Builds and serves an ACORN-gamma index (stanford-futuredata/ACORN, a faiss fork) for ann.py.
//
//   acorn_bench build CORPUS32.npy INDEX M GAMMA M_BETA FROM TO
//       inserts entries FROM..TO of the index's insertion sequence into INDEX (created when FROM is 0, read back
//       otherwise), with L2 over the unit vectors, and writes it back; a build runs as several such pieces.
//   acorn_bench serve INDEX QUERIES.npy
//       reads commands on stdin, one per line:
//         vis NAME IDS.npy     the visible set NAME from sorted int32 ids, as ACORN's per-item filter map
//         q QI K EF NAME       the K nearest visible items to query QI, efSearch EF; NAME "all" is unfiltered
//       and answers each q with the search time in ms (f64), the id count (i64) and the ids (i64), in binary.
#include <faiss/IndexACORN.h>
#include <faiss/impl/AuxIndexStructures.h>
#include <faiss/index_io.h>
#include <omp.h>

#include <sys/resource.h>
#include <sys/time.h>
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

static double now_ms() {
    timeval t;
    gettimeofday(&t, nullptr);
    return t.tv_sec * 1e3 + t.tv_usec * 1e-3;
}

static double peak_rss_gib() {
    rusage r;
    getrusage(RUSAGE_SELF, &r);
    return r.ru_maxrss / 1048576.0;
}

// A little-endian .npy file: its dtype descr, shape, and the offset of its data.
struct Npy {
    std::string descr;
    std::vector<size_t> shape;
    size_t off;
};

static Npy npy_header(const char* path) {
    std::ifstream f(path, std::ios::binary);
    char magic[8];
    f.read(magic, 8);
    if (std::memcmp(magic, "\x93NUMPY", 6) != 0) throw std::runtime_error(std::string(path) + " is not a .npy file");
    size_t hl;
    if (magic[6] == 1) {
        uint16_t h;
        f.read((char*)&h, 2);
        hl = h;
    } else {
        uint32_t h;
        f.read((char*)&h, 4);
        hl = h;
    }
    std::string h(hl, ' ');
    f.read(&h[0], hl);
    Npy n;
    n.off = (magic[6] == 1 ? 10 : 12) + hl;
    auto d = h.find("'descr': '") + 10;
    n.descr = h.substr(d, h.find('\'', d) - d);
    if (h.find("'fortran_order': False") == std::string::npos) throw std::runtime_error("fortran order unsupported");
    auto s = h.find('(', h.find("'shape'")) + 1, e = h.find(')', s);
    std::stringstream ss(h.substr(s, e - s));
    std::string tok;
    while (std::getline(ss, tok, ','))
        if (tok.find_first_not_of(' ') != std::string::npos) n.shape.push_back(std::stoull(tok));
    return n;
}

template <class T>
static std::vector<T> npy_rows(const char* path, const Npy& n, size_t row_len, size_t from, size_t to) {
    std::vector<T> v((to - from) * row_len);
    std::ifstream f(path, std::ios::binary);
    f.seekg(n.off + from * row_len * sizeof(T));
    f.read((char*)v.data(), v.size() * sizeof(T));
    if (!f) throw std::runtime_error(std::string("short read from ") + path);
    return v;
}

int main(int argc, char** argv) {
    std::string cmd = argc > 1 ? argv[1] : "";
    if (cmd == "build" && argc == 9) {
        // ACORN's add() works in one call: a second call on a non-empty index builds a markedly worse graph, more
        // slowly. So the first piece stores every vector and assigns every level, as add() would, and each piece
        // inserts its share of add()'s insertion sequence (levels top down, each level in add()'s random order).
        const char* corpus = argv[2];
        std::string out = argv[3];
        int M = atoi(argv[4]), gamma = atoi(argv[5]), mbeta = atoi(argv[6]);
        size_t from = std::stoull(argv[7]), to = std::stoull(argv[8]);
        Npy h = npy_header(corpus);
        if (h.descr != "<f4" || h.shape.size() != 2) throw std::runtime_error("corpus must be f32 rows");
        size_t N = h.shape[0], d = h.shape[1];
        to = std::min(to, N);
        std::vector<int> metadata(N, 0);  // ACORN-gamma's build does not read predicates; its debug output does
        double t0 = now_ms();
        faiss::IndexACORNFlat* ix;
        if (from == 0) {
            ix = new faiss::IndexACORNFlat(d, M, gamma, metadata, mbeta, faiss::METRIC_L2);
            auto x = npy_rows<float>(corpus, h, d, 0, N);
            ix->storage->add(N, x.data());
            ix->ntotal = N;
            ix->acorn.prepare_level_tab(N, false);
        } else {
            ix = dynamic_cast<faiss::IndexACORNFlat*>(faiss::read_index(out.c_str()));
            if (!ix || (size_t)ix->ntotal != N) throw std::runtime_error("the index is not this corpus's");
        }
        faiss::ACORN& acorn = ix->acorn;
        acorn.metadata = metadata.data();
        double t1 = now_ms();
        // add()'s insertion sequence: bucket by level, then each level shuffled by its generator, top level first
        std::vector<int> hist, seq(N);
        for (size_t i = 0; i < N; i++) {
            int l = acorn.levels[i] - 1;
            while (l >= (int)hist.size()) hist.push_back(0);
            hist[l]++;
        }
        std::vector<int> offs(hist.size() + 1, 0);
        for (size_t l = 0; l + 1 < hist.size(); l++) offs[l + 1] = offs[l] + hist[l];
        for (size_t i = 0; i < N; i++) seq[offs[acorn.levels[i] - 1]++] = i;
        faiss::RandomGenerator rng2(789);
        std::vector<std::pair<size_t, size_t>> runs;  // [i0, i1) of seq for each level, in insertion order
        size_t i1 = N;
        for (int l = hist.size() - 1; l >= 0; l--) {
            size_t i0 = i1 - hist[l];
            for (size_t j = i0; j < i1; j++) std::swap(seq[j], seq[j + rng2.rand_int(i1 - j)]);
            runs.push_back({i0, i1});
            i1 = i0;
        }
        std::vector<omp_lock_t> locks(N);
        for (auto& l : locks) omp_init_lock(&l);
        size_t pos = 0, done = 0;  // position in the insertion sequence
        for (auto [a, b] : runs) {
            size_t lo = std::max(a, a + (from > pos ? from - pos : 0)), hi = std::min(b, a + (to > pos ? to - pos : 0));
            int pt_level = acorn.levels[seq[a]] - 1;
            if (lo < hi) {
#pragma omp parallel if (hi > lo + 100)
                {
                    faiss::VisitedTable vt(N);
                    std::unique_ptr<faiss::DistanceComputer> dis(ix->storage->get_distance_computer());
#pragma omp for schedule(static)
                    for (size_t i = lo; i < hi; i++) {
                        dis->set_query(static_cast<faiss::IndexFlat*>(ix->storage)->get_xb() + (size_t)seq[i] * d);
                        acorn.add_with_locks(*dis, pt_level, seq[i], locks, vt);
                    }
                }
                done += hi - lo;
            }
            pos += b - a;
        }
        for (auto& l : locks) omp_destroy_lock(&l);
        double t2 = now_ms();
        faiss::write_index(ix, (out + ".tmp").c_str());
        std::rename((out + ".tmp").c_str(), out.c_str());
        printf("insertions %zu..%zu (%zu done): start %.1f s, insert %.1f s, write %.1f s; peak RSS %.2f GiB\n", from, to,
               done, (t1 - t0) / 1e3, (t2 - t1) / 1e3, (now_ms() - t2) / 1e3, peak_rss_gib());
        return 0;
    }
    if (cmd == "serve" && argc == 4) {
        double t0 = now_ms();
        auto* ix = dynamic_cast<faiss::IndexACORNFlat*>(faiss::read_index(argv[2]));
        if (!ix) throw std::runtime_error("not an ACORN index");
        size_t N = ix->ntotal, d = ix->d;
        std::vector<int> metadata(N, 0);
        ix->acorn.metadata = metadata.data();
        Npy qh = npy_header(argv[3]);
        if (qh.descr != "<f8" || qh.shape[1] != d) throw std::runtime_error("queries must be f64 rows of the index's width");
        auto q64 = npy_rows<double>(argv[3], qh, d, 0, qh.shape[0]);
        std::vector<float> q(q64.begin(), q64.end());
        fprintf(stderr, "acorn: loaded %zu rows in %.1f s; M %d gamma %d M_beta %d; peak RSS %.2f GiB\n", N,
                (now_ms() - t0) / 1e3, ix->acorn.M, ix->acorn.gamma, ix->acorn.M_beta, peak_rss_gib());
        std::map<std::string, std::vector<char>> vis;
        vis["all"] = std::vector<char>(N, 1);
        std::string line;
        while (std::getline(std::cin, line)) {
            std::stringstream ss(line);
            std::string op;
            ss >> op;
            if (op == "vis") {
                std::string name, path;
                ss >> name >> path;
                Npy h = npy_header(path.c_str());
                if (h.descr != "<i4") throw std::runtime_error("visible sets hold int32 ids");
                auto ids = npy_rows<int32_t>(path.c_str(), h, 1, 0, h.shape[0]);
                std::vector<char> m(N, 0);
                for (int32_t i : ids) m[i] = 1;
                vis[name] = std::move(m);
                continue;
            }
            size_t qi, k;
            int ef;
            std::string name;
            ss >> qi >> k >> ef >> name;
            faiss::SearchParametersACORN p;
            p.efSearch = ef;
            ix->acorn.efSearch = ef;  // the beam's size is read from the index; the parameter sets only the stopping rule
            std::vector<faiss::idx_t> I(k);
            std::vector<float> D(k);
            double s = now_ms();
            ix->search(1, q.data() + qi * d, k, D.data(), I.data(), vis.at(name).data(), &p);
            int64_t n = 0;
            for (size_t j = 0; j < k; j++)
                if (I[j] >= 0) I[n++] = I[j];
            double ms = now_ms() - s;
            fwrite(&ms, 8, 1, stdout);
            fwrite(&n, 8, 1, stdout);
            fwrite(I.data(), 8, n, stdout);
            fflush(stdout);
        }
        return 0;
    }
    fprintf(stderr, "usage: acorn_bench build CORPUS32.npy INDEX M GAMMA M_BETA FROM TO | serve INDEX QUERIES.npy\n");
    return 2;
}
