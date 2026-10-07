"""Abre un DXF con ezdxf (lector independiente), lo audita y lo pinta a PNG.

Uso:  python3 ver_dxf.py entrada.dxf salida.png
Necesita: pip install ezdxf matplotlib
"""
import sys
from collections import Counter

import ezdxf
from ezdxf.addons.drawing import Frontend, RenderContext
from ezdxf.addons.drawing.matplotlib import MatplotlibBackend
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

doc = ezdxf.readfile(sys.argv[1])
msp = doc.modelspace()
print("version:", doc.dxfversion, "| entidades:", dict(Counter(e.dxftype() for e in msp)),
      "| capas:", sorted({e.dxf.layer for e in msp}))
aud = doc.audit()
print("errores de auditoria:", len(aud.errors), "| arreglos:", len(aud.fixes))
fig = plt.figure(figsize=(16, 2.2))
ax = fig.add_axes([0, 0, 1, 1])
Frontend(RenderContext(doc), MatplotlibBackend(ax)).draw_layout(msp, finalize=True)
fig.savefig(sys.argv[2], dpi=110)
print("pintado en", sys.argv[2])
