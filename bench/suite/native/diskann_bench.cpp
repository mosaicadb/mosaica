// Builds and serves DiskANN's SSD index (microsoft/DiskANN, C++ at tag 0.7.0, the release diskannpy 0.7.0 wraps) for
// bench/suite. install/diskann.sh builds it.
//
//   diskann_bench build CORPUS.bin PREFIX R L SEARCH_RAM_GB BUILD_RAM_GB THREADS
//       build_disk_index<float> with L2 over the unit vectors (the same order as inner product): graph degree R,
//       build list L, PQ codes sized to SEARCH_RAM_GB in memory at search time.
//   diskann_bench serve PREFIX QUERIES.npy
//       loads the index (PQ codes and the medoids in memory, the graph and the full vectors read from the file with
//       Linux AIO, no node cache) and reads commands on stdin, one per line:
//         q QI K L BEAM           the K nearest to query QI (QUERIES.npy holds f64 rows), search list L, beam width
//                                 BEAM
//       and answers each with the search time in ms (f64), the id count (i64) and the ids (i64), in binary, as
//       ../../threshold-db/acorn_bench.cpp does.
#include <omp.h>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <memory>
#include <sstream>
#include <string>
#include <sys/time.h>
#include <unistd.h>
#include <vector>

#include "disk_utils.h"
#include "linux_aligned_file_reader.h"
#include "pq_flash_index.h"

static double now_ms() {
    timeval t;
    gettimeofday(&t, nullptr);
    return t.tv_sec * 1e3 + t.tv_usec * 1e-3;
}

// f64 rows of a little-endian .npy file
static std::vector<float> npy_f64_rows(const char* path, size_t& rows, size_t& cols) {
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
    if (h.find("'<f8'") == std::string::npos) throw std::runtime_error("queries must be f64");
    auto s = h.find('(', h.find("'shape'")) + 1, e = h.find(')', s);
    std::stringstream ss(h.substr(s, e - s));
    std::string tok;
    std::vector<size_t> shape;
    while (std::getline(ss, tok, ','))
        if (tok.find_first_not_of(' ') != std::string::npos) shape.push_back(std::stoull(tok));
    rows = shape[0];
    cols = shape[1];
    std::vector<double> d(rows * cols);
    f.read((char*)d.data(), d.size() * 8);
    return std::vector<float>(d.begin(), d.end());
}

int main(int argc, char** argv) {
    std::string cmd = argc > 1 ? argv[1] : "";
    if (cmd == "build" && argc == 9) {
        std::string params = std::string(argv[4]) + " " + argv[5] + " " + argv[6] + " " + argv[7] + " " + argv[8] +
                             " 0 0 0 0";  // R L B M T, no disk PQ, no reorder data, no build PQ, QD default
        double t0 = now_ms();
        int r = diskann::build_disk_index<float>(argv[2], argv[3], params.c_str(), diskann::Metric::L2);
        fprintf(stderr, "diskann: built in %.1f s\n", (now_ms() - t0) / 1e3);
        return r;
    }
    if (cmd == "serve" && argc == 4) {
        // the answers go to the original stdout; everything DiskANN prints (to stdout too) goes to stderr
        FILE* out = fdopen(dup(1), "wb");
        dup2(2, 1);
        std::shared_ptr<AlignedFileReader> reader(new LinuxAlignedFileReader());
        auto ix = std::make_unique<diskann::PQFlashIndex<float, uint32_t>>(reader, diskann::Metric::L2);
        // two I/O contexts although one thread searches: with one, load() deadlocks in 0.7.0
        // (use_medoids_data_as_centroids holds the only context while read_nodes waits for one)
        if (ix->load(2, argv[2]) != 0) throw std::runtime_error("cannot load the index");
        size_t nq, d;
        auto q = npy_f64_rows(argv[3], nq, d);
        size_t ad = ix->get_data_dim();
        std::vector<float> qa;
        float* qbuf;
        if (posix_memalign((void**)&qbuf, 32, ad * sizeof(float)) != 0) throw std::runtime_error("no memory");
        omp_set_num_threads(1);
        fprintf(stderr, "diskann: loaded %s, %zu queries of %zu dimensions (index %zu)\n", argv[2], nq, d, ad);
        std::string line;
        while (std::getline(std::cin, line)) {
            std::stringstream ss(line);
            std::string op;
            size_t qi, k, L, beam;
            ss >> op >> qi >> k >> L >> beam;
            if (op != "q") continue;
            std::memset(qbuf, 0, ad * sizeof(float));
            std::memcpy(qbuf, q.data() + qi * d, d * sizeof(float));
            std::vector<uint64_t> ids(k);
            std::vector<float> dist(k);
            double s = now_ms();
            ix->cached_beam_search(qbuf, k, L, ids.data(), dist.data(), beam);
            double ms = now_ms() - s;
            std::vector<int64_t> res;
            for (size_t j = 0; j < k; j++)
                if (ids[j] != (uint64_t)-1 && dist[j] < 3.4e38f) res.push_back((int64_t)ids[j]);
            int64_t n = res.size();
            fwrite(&ms, 8, 1, out);
            fwrite(&n, 8, 1, out);
            fwrite(res.data(), 8, n, out);
            fflush(out);
        }
        return 0;
    }
    fprintf(stderr, "usage: diskann_bench build CORPUS.bin PREFIX R L SEARCH_RAM_GB BUILD_RAM_GB THREADS | "
                    "serve PREFIX QUERIES.npy\n");
    return 2;
}
