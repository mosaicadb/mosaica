# Additive stream codes: the general code the nibble decoder accepts.
#
# A block's code at a rise is the sum of the pulses of the nibbles read so far. The decoder is unchanged: the block's
# estimate is kappa * sum over nibbles of T[nibble], T[v] = <w, pulse of v>, one 16-entry lookup per nibble.
#
# Stream: one nibble per `p` positions (4/p bits per position), all read at one rise. Nibble t adds a pulse to the
#   positions of steps t, ..., t+J-1:  c[step t] = G_0[n_t] + G_1[n_(t-1)] + ... + G_(J-1)[n_(t-J+1)], G_i 16 x p.
#   J = 1 is a product codebook per nibble (with Lloyd codebooks and p = 2 this is BlockQuant's construction,
#   arXiv:2605.19972). J >= 2 is a trellis code whose sliding-window decoder is constrained to be a sum over aligned
#   nibbles. kind "free" is the unconstrained window over two nibbles (Mao-Gray-Linder's sliding-block decoder, as QTIP
#   uses it): a 256-entry table, here as a reference only.
# Phased: the same with J = 2 and a period of P steps, each phase with its own pulses and its own rise. Nibbles of
#   different rises alternate along the block, so a later rise's nibbles straddle the earlier rise's, and the code read
#   to rise r is the sum of the pulses of the phases with rise <= r.
# Encoders: a Viterbi search per stream (16^(J-1) states) on the weighted sum over rises of |x - code at the rise|^2,
#   repeated over the streams (coordinate descent). Within a Phased stream the search over all its rises is exact.
import numpy as np

NONE = 16  # index of "no nibble yet" (its pulse is zero)


def _prev(n, i):
    N, T = n.shape
    prev = np.full((N, T), NONE, dtype=np.int64)
    prev[:, i:] = n[:, :-i]
    return prev


class Stream:
    def __init__(self, p, J, rise, kind="add", seed=0, init=None, scale=1.0, off=0):
        # off: the stream's nibbles start at position `off` and wrap round the block (the positions of a rotated block
        # are exchangeable).
        self.p, self.J, self.rise, self.kind, self.off = p, J, rise, kind, off
        rng = np.random.default_rng(seed)
        G = np.zeros((J, 17, p), dtype=np.float64)
        G[0, :16] = init if init is not None else rng.standard_normal((16, p)) * scale
        for i in range(1, J):
            G[i, :16] = rng.standard_normal((16, p)) * 0.05 * scale
        self.G = G
        self.W = None

    def set_free(self):
        self.W = self.out().copy()
        self.kind = "free"

    def out(self):
        """Branch outputs: array (17,)*J + (p,), index order n_t, n_(t-1), ..."""
        if self.kind == "free":
            return self.W
        G = self.G
        if self.J == 1:
            return G[0]
        if self.J == 2:
            return G[0][:, None, :] + G[1][None, :, :]
        return G[0][:, None, None, :] + G[1][None, :, None, :] + G[2][None, None, :, :]

    def decode(self, n, m, r=None):
        """Code at rise r (all of it if r >= the stream's rise, else zero)."""
        N, T = n.shape
        if r is not None and r < self.rise:
            return np.zeros((N, m))
        c = self.out()[tuple([n] + [_prev(n, i) for i in range(1, self.J)])]
        full = np.zeros((N, m))
        full[:, : T * self.p] = c.reshape(N, T * self.p)
        return np.roll(full, self.off, axis=1) if self.off else full

    @staticmethod
    def _collapse(targets):
        num, den = 0.0, 0.0
        for _, w, tg in targets:
            num = num + w * tg
            den += w
        return num / den

    def encode(self, targets):
        """targets: list of (rise, weight, target (N, m)) for the rises that contain the stream. Viterbi."""
        X = self._collapse(targets)
        N, m = X.shape
        p, J = self.p, self.J
        T = m // p
        if self.off:
            X = np.roll(X, -self.off, axis=1)
        Xs = X[:, : T * p].reshape(N, T, p).astype(np.float32)
        O = self.out().astype(np.float32)
        n = np.zeros((N, T), dtype=np.int64)
        ar = np.arange(N)
        if J == 1:
            C = O[:16]
            cn = (C * C).sum(1)
            for t in range(T):
                n[:, t] = (cn[None, :] - 2.0 * Xs[:, t] @ C.T).argmin(1)
            return n
        if J == 2:
            O2 = O[:16]                                  # (16 a, 17 b, p)
            on = (O2 * O2).sum(-1)
            flat = O2.reshape(16 * 17, p)
            cost, bps = None, []
            for t in range(T):
                met = on[None] - 2.0 * (Xs[:, t] @ flat.T).reshape(N, 16, 17)   # (N, a, b)
                if t == 0:
                    cost = met[:, :, NONE]
                    bps.append(None)
                else:
                    tot = met[:, :, :16] + cost[:, None, :]
                    bp = tot.argmin(2)
                    cost = np.take_along_axis(tot, bp[:, :, None], 2)[:, :, 0]
                    bps.append(bp)
            a = cost.argmin(1)
            for t in range(T - 1, -1, -1):
                n[:, t] = a
                if t > 0:
                    a = bps[t][ar, a]
            return n
        O3 = O[:16]                                      # (16 a, 17 b, 17 c, p); state (n_(t-1), n_(t-2))
        on = (O3 * O3).sum(-1)
        flat = O3.reshape(16 * 17 * 17, p)
        cost, bps = None, []
        for t in range(T):
            met = on[None] - 2.0 * (Xs[:, t] @ flat.T).reshape(N, 16, 17, 17)  # (N, a, b, c)
            if t == 0:
                cost = met[:, :, NONE, NONE][:, :, None]
                bps.append(None)
            elif t == 1:
                cost = met[:, :, :16, NONE] + cost[:, None, :, 0]                # (N, a, b)
                bps.append(None)
            else:
                tot = met[:, :, :16, :16] + cost[:, None, :, :]                  # cost[b, c] + met[a, b, c]
                bp = tot.argmin(3).astype(np.uint8)
                cost = np.take_along_axis(tot, bp[..., None].astype(np.int64), 3)[..., 0]
                bps.append(bp)
        if T == 1:
            n[:, 0] = cost[:, :, 0].argmin(1)
            return n
        fl = cost.reshape(N, -1).argmin(1)
        a, b = fl // 16, fl % 16
        for t in range(T - 1, -1, -1):
            n[:, t] = a
            if t >= 2:
                c = bps[t][ar, a, b].astype(np.int64)
                a, b = b, c
            elif t == 1:
                a = b
        return n

    def fit(self, n, targets):
        """Least-squares pulses (kind add) or cell means (kind free)."""
        target = self._collapse(targets)
        N, T = n.shape
        p, J = self.p, self.J
        if self.off:
            target = np.roll(target, -self.off, axis=1)
        Y = target[:, : T * p].reshape(N * T, p)
        idx = [n.reshape(-1)] + [_prev(n, i).reshape(-1) for i in range(1, J)]
        if self.kind == "free":
            W = self.W.copy()
            flat = idx[0]
            for i in range(1, J):
                flat = flat * 17 + idx[i]
            size = 17 ** J
            cnt = np.bincount(flat, minlength=size)
            ok = cnt > 0
            for j in range(p):
                s = np.bincount(flat, weights=Y[:, j], minlength=size)
                Wj = W[..., j].reshape(-1).copy()
                Wj[ok] = s[ok] / cnt[ok]
                W[..., j] = Wj.reshape(W.shape[:-1])
            self.W = W
            return
        Z = np.zeros((N * T, 16 * J), dtype=np.float32)
        rows = np.arange(N * T)
        for i in range(J):
            ok = idx[i] < 16
            Z[rows[ok], 16 * i + idx[i][ok]] = 1.0
        A = (Z.T @ Z).astype(np.float64)
        B = (Z.T @ Y.astype(np.float32)).astype(np.float64)
        sol = np.linalg.pinv(A + 1e-9 * np.eye(16 * J)) @ B
        for i in range(J):
            self.G[i, :16] = sol[16 * i: 16 * (i + 1)]


class Phased:
    """Period P, J = 2 or 3: phase f = t mod P has pulses G[0][f] (its own step), G[1][f] (the next step), G[2][f]
    (the step after, J = 3) and rise rho[f]. rho_steps, if given, overrides the rise step by step (the pulses stay by
    phase): it lets a sub-bit rise read the nibbles of the first share of the block only."""

    def __init__(self, p, rho, J=2, seed=0, rho_steps=None):
        self.p, self.rho, self.P, self.J = p, list(rho), len(rho), J
        self.rho_steps = rho_steps
        self.rise = min(rho) if rho_steps is None else int(min(rho_steps))
        rng = np.random.default_rng(seed)
        self.G = np.zeros((J, self.P, 17, p))
        self.G[:, :, :16] = rng.standard_normal((J, self.P, 16, p)) * 0.02

    # the earlier two-pulse interface
    @property
    def G0(self):
        return self.G[0]

    @property
    def G1(self):
        return self.G[1]

    def _rho(self, T):
        if self.rho_steps is not None:
            return np.asarray(self.rho_steps[:T])
        return np.array([self.rho[t % self.P] for t in range(T)])

    def decode(self, n, m, r=None):
        N, T = n.shape
        p = self.p
        ph = np.arange(T) % self.P
        rho = self._rho(T)
        inc = np.ones(T) if r is None else (rho <= r).astype(float)
        tot = np.zeros((N, T, p))
        for i in range(self.J):
            ni = n if i == 0 else _prev(n, i)
            phi = np.concatenate((np.zeros(i, dtype=np.int64), ph[: T - i]))
            inci = np.concatenate((np.zeros(i), inc[: T - i]))
            tot += self.G[i][phi[None, :], ni] * inci[None, :, None]
        full = np.zeros((N, m))
        full[:, : T * p] = tot.reshape(N, T * p)
        return full

    def _outs(self, t, rho, rises):
        """For step t: per rise in `rises`, the branch output tensor over (n_t, n_(t-1)[, n_(t-2)])."""
        outs = []
        for r in rises:
            parts = []
            for i in range(self.J):
                if t - i >= 0 and rho[t - i] <= r:
                    parts.append(self.G[i][(t - i) % self.P])
                else:
                    parts.append(np.zeros((17, self.p)))
            if self.J == 2:
                outs.append(parts[0][:16, None, :] + parts[1][None, :, :])
            else:
                outs.append(parts[0][:16, None, None, :] + parts[1][None, :, None, :] + parts[2][None, None, :, :])
        return outs

    def encode(self, targets):
        N, m = targets[0][2].shape
        p, J = self.p, self.J
        T = m // p
        rho = self._rho(T)
        rises = [r for r, _, _ in targets]
        ws = [w for _, w, _ in targets]
        Xr = [tg[:, : T * p].reshape(N, T, p).astype(np.float32) for _, _, tg in targets]
        n = np.zeros((N, T), dtype=np.int64)
        ar = np.arange(N)
        shape = (N, 16, 17) if J == 2 else (N, 16, 17, 17)
        cache = {}
        cost, bps = None, []
        for t in range(T):
            key = (t % self.P, tuple(rho[max(t - J + 1, 0): t + 1]), min(t, J - 1))
            if key not in cache:
                outs = self._outs(t, rho, rises)
                cache[key] = [(o.reshape(-1, p).astype(np.float32), (o * o).sum(-1).astype(np.float32)) for o in outs]
            met = np.zeros(shape, dtype=np.float32)
            for (flat, on), w, X in zip(cache[key], ws, Xr):
                met += w * (on[None] - 2.0 * (X[:, t] @ flat.T).reshape(shape))
            if J == 2:
                if t == 0:
                    cost = met[:, :, NONE]
                    bps.append(None)
                else:
                    tot = met[:, :, :16] + cost[:, None, :]
                    bp = tot.argmin(2)
                    cost = np.take_along_axis(tot, bp[:, :, None], 2)[:, :, 0]
                    bps.append(bp)
            else:
                if t == 0:
                    cost = met[:, :, NONE, NONE][:, :, None]
                    bps.append(None)
                elif t == 1:
                    cost = met[:, :, :16, NONE] + cost[:, None, :, 0]
                    bps.append(None)
                else:
                    tot = met[:, :, :16, :16] + cost[:, None, :, :]
                    bp = tot.argmin(3).astype(np.uint8)
                    cost = np.take_along_axis(tot, bp[..., None].astype(np.int64), 3)[..., 0]
                    bps.append(bp)
        if J == 2:
            a = cost.argmin(1)
            for t in range(T - 1, -1, -1):
                n[:, t] = a
                if t > 0:
                    a = bps[t][ar, a]
            return n
        fl = cost.reshape(N, -1).argmin(1)
        a, b = fl // 16, fl % 16
        for t in range(T - 1, -1, -1):
            n[:, t] = a
            if t >= 2:
                c = bps[t][ar, a, b].astype(np.int64)
                a, b = b, c
            elif t == 1:
                a = b
        return n

    def fit(self, n, targets):
        N, T = n.shape
        p, P, J = self.p, self.P, self.J
        rho = self._rho(T)
        ph = np.arange(T) % P
        nu = 16 * J * P
        A = np.zeros((nu, nu))
        B = np.zeros((nu, p))
        rows = np.arange(N * T)
        for r, w, tg in targets:
            Y = tg[:, : T * p].reshape(N * T, p).astype(np.float32)
            Z = np.zeros((N * T, nu), dtype=np.float32)
            for i in range(J):
                ni = (n if i == 0 else _prev(n, i)).reshape(-1)
                phi = np.tile(np.concatenate((np.zeros(i, dtype=np.int64), ph[: T - i])), N)
                inci = np.tile(np.concatenate((np.zeros(i, dtype=bool), rho[: T - i] <= r)), N)
                ok = inci & (ni < 16)
                Z[rows[ok], 16 * (J * phi[ok] + i) + ni[ok]] = 1.0
            A += w * (Z.T @ Z)
            B += w * (Z.T @ Y)
        sol = np.linalg.pinv(A + 1e-9 * np.trace(A) / nu * np.eye(nu)) @ B
        for f in range(P):
            for i in range(J):
                self.G[i, f, :16] = sol[16 * (J * f + i): 16 * (J * f + i) + 16]


class Nested:
    """Streams and rises. emphasis: per rise, multiplied by 1 / D_rise (equal emphasis is the built encoder's sum of
    log tan^2)."""

    def __init__(self, streams, nrise, emphasis=None):
        self.streams, self.nrise = streams, nrise
        self.emph = np.ones(nrise) if emphasis is None else np.asarray(emphasis, dtype=float)
        self.w = self.emph.copy()

    def codes_at(self, nibs, m):
        """codes[k][r]: stream k's code at rise r."""
        return [[s.decode(n, m, r) for r in range(self.nrise)] for s, n in zip(self.streams, nibs)]

    def prefix(self, codes, r):
        return sum(c[r] for c in codes)

    def targets(self, X, codes, k):
        s = self.streams[k]
        out = []
        for r in range(s.rise, self.nrise):
            others = sum(c[r] for j, c in enumerate(codes) if j != k)
            out.append((r, self.w[r], X - others))
        return out

    def encode(self, X, rounds=2):
        N, m = X.shape
        zero = [np.zeros_like(X) for _ in range(self.nrise)]
        codes = [zero for _ in self.streams]
        nibs = [None] * len(self.streams)
        for it in range(rounds + 1):
            for k, s in enumerate(self.streams):
                if it > 0 and len(self.streams) == 1:
                    break
                n = s.encode(self.targets(X, codes, k))
                nibs[k] = n
                codes[k] = [s.decode(n, m, r) for r in range(self.nrise)]
        return nibs, codes

    def dist(self, X, codes):
        return np.array([float(((X - self.prefix(codes, r)) ** 2).mean()) for r in range(self.nrise)])

    def train(self, X, iters=12, rounds=1, log=None):
        m = X.shape[1]
        D = None
        for it in range(iters):
            nibs, codes = self.encode(X, rounds)
            for k, s in enumerate(self.streams):
                s.fit(nibs[k], self.targets(X, codes, k))
                codes[k] = [s.decode(nibs[k], m, r) for r in range(self.nrise)]
            D = self.dist(X, codes)
            self.w = self.emph / np.maximum(D, 1e-12)
            if log:
                log(f"    iter {it}: D per rise {np.round(D, 5)}")
        return D

    def tan2(self, Y, rounds=2, scales=(1.0,)):
        """tan^2 per (item, rise) on blocks Y, at the item's rms times the best of `scales` (by sum of log tan^2)."""
        from clib import tan2 as t2f
        rms = np.sqrt((Y * Y).mean(1, keepdims=True))
        best = np.full(len(Y), np.inf)
        out = np.zeros((len(Y), self.nrise))
        for sc in scales:
            X = Y / (sc * rms)
            _, codes = self.encode(X, rounds)
            t2 = np.stack([t2f(self.prefix(codes, r), Y) for r in range(self.nrise)], axis=1)
            obj = np.log(np.maximum(t2, 1e-300)).sum(1)
            bt = obj < best
            best[bt] = obj[bt]
            out[bt] = t2[bt]
        return out
