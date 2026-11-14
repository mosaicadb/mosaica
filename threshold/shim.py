# The probe venv has no scipy; combined/core.py needs only scipy.special.gammaln. Install a stand-in built on math.lgamma.
import sys, types, math, numpy as np
_lg = np.vectorize(math.lgamma, otypes=[np.float64])
sp = types.ModuleType("scipy"); spec = types.ModuleType("scipy.special"); spec.gammaln = lambda x: _lg(np.asarray(x, np.float64))
sp.special = spec; sys.modules.setdefault("scipy", sp); sys.modules.setdefault("scipy.special", spec)
