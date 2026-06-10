# -*- coding: utf-8 -*-
"""
pdf_to_excel — convierte reportes PDF "Lista de órdenes" (Bystronic) a Excel.

Regla: solo procesa PDFs cuyo nombre EMPIEZA por 'LD' o 'LP' (mayúsc/minúsc).

Uso (desde código o desde el .exe portable):
  pdf_to_excel.exe                      -> convierte los LP*/LD* de la carpeta actual
  pdf_to_excel.exe archivo.pdf          -> convierte 1 PDF
  pdf_to_excel.exe C:\\ruta\\carpeta      -> convierte todos los LP*/LD* de la carpeta
  pdf_to_excel.exe a.pdf b.pdf ...      -> convierte varios (arrastrar y soltar al .exe)
  pdf_to_excel.exe -o C:\\salida carpeta -> carpeta de salida distinta
  pdf_to_excel.exe --watch C:\\entrada    -> AUTOMÁTICO: vigila la carpeta y convierte
                                            cada PDF LP*/LD* nuevo en cuanto aparece

El .xlsx se guarda junto al PDF (o en -o), con el mismo nombre base.
"""
import sys
import os
import time
import argparse
import glob

# Permite ejecutar tanto como módulo (paquete) como script suelto / .exe.
try:
    from .parser import parse_pdf
    from .writer import write_xlsx
except ImportError:  # ejecución directa o PyInstaller
    from parser import parse_pdf
    from writer import write_xlsx

PREFIJOS = ('LD', 'LP')


def tiene_prefijo(nombre):
    base = os.path.basename(nombre)
    return base[:2].upper() in PREFIJOS


def convertir(pdf_path, out_dir=None):
    """Convierte un PDF a .xlsx. Devuelve (ruta_salida|None, estado)."""
    if not tiene_prefijo(pdf_path):
        return None, 'omitido (no empieza por LD/LP)'
    try:
        data = parse_pdf(pdf_path)
    except Exception as e:
        return None, f'ERROR al leer PDF: {e}'
    base = os.path.splitext(os.path.basename(pdf_path))[0]
    out_dir = out_dir or os.path.dirname(os.path.abspath(pdf_path))
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, base + '.xlsx')
    try:
        write_xlsx(data, out_path)
    except Exception as e:
        return None, f'ERROR al escribir Excel: {e}'
    return out_path, 'OK'


def _listar_pdfs(carpeta):
    return sorted(set(glob.glob(os.path.join(carpeta, '*.pdf')) +
                      glob.glob(os.path.join(carpeta, '*.PDF'))))


def _convertir_lista(pdfs, out_dir):
    n_ok = 0
    for pdf in pdfs:
        out, estado = convertir(pdf, out_dir)
        nombre = os.path.basename(pdf)
        if estado == 'OK':
            n_ok += 1
            print(f'  [OK]      {nombre}  ->  {os.path.basename(out)}')
        elif estado.startswith('omitido'):
            print(f'  [omitido] {nombre}  ({estado})')
        else:
            print(f'  [FALLO]   {nombre}  {estado}')
    return n_ok


def modo_watch(carpeta, out_dir, intervalo=2.0):
    """Vigila `carpeta` y convierte cada PDF LP*/LD* nuevo o modificado."""
    print(f'AUTOMATICO: vigilando "{os.path.abspath(carpeta)}" (Ctrl+C para salir)\n')
    vistos = {}  # ruta -> mtime ya procesado
    try:
        while True:
            for pdf in _listar_pdfs(carpeta):
                if not tiene_prefijo(pdf):
                    continue
                try:
                    mt = os.path.getmtime(pdf)
                except OSError:
                    continue
                xlsx = os.path.splitext(pdf)[0] + '.xlsx'
                # Procesa si es nuevo, si cambió, o si falta el .xlsx
                if vistos.get(pdf) == mt and os.path.exists(xlsx):
                    continue
                out, estado = convertir(pdf, out_dir)
                ts = time.strftime('%H:%M:%S')
                if estado == 'OK':
                    print(f'  [{ts}] {os.path.basename(pdf)}  ->  {os.path.basename(out)}')
                    vistos[pdf] = mt
                elif not estado.startswith('omitido'):
                    print(f'  [{ts}] FALLO {os.path.basename(pdf)}: {estado}')
            time.sleep(intervalo)
    except KeyboardInterrupt:
        print('\nDetenido.')


def main(argv=None):
    ap = argparse.ArgumentParser(
        description='Convierte PDF (nombre LD*/LP*) a Excel con el layout del reporte.')
    ap.add_argument('entrada', nargs='*',
                    help='PDF(s) o carpeta (def: carpeta actual)')
    ap.add_argument('-o', '--salida', default=None,
                    help='Carpeta de salida (def: junto al PDF)')
    ap.add_argument('-w', '--watch', metavar='CARPETA', default=None,
                    help='Modo automatico: vigila la carpeta y convierte PDFs nuevos')
    ap.add_argument('--no-pause', action='store_true',
                    help='No esperar tecla al terminar (para scripts)')
    args = ap.parse_args(argv)

    if args.watch:
        if not os.path.isdir(args.watch):
            print(f'No es carpeta: {args.watch}')
            return 2
        modo_watch(args.watch, args.salida)
        return 0

    entradas = args.entrada or ['.']

    rc = 0
    pdfs = []
    for e in entradas:
        if os.path.isdir(e):
            pdfs += _listar_pdfs(e)
        elif os.path.isfile(e):
            pdfs.append(e)
        else:
            print(f'No existe: {e}')
            rc = 2

    if not pdfs:
        print('No se encontraron PDFs para convertir.')
        _pausa_si_interactivo(args.no_pause)
        return rc or 1

    print(f'Convirtiendo {len(pdfs)} PDF(s)...\n')
    n_ok = _convertir_lista(pdfs, args.salida)
    print(f'\nHecho: {n_ok}/{len(pdfs)} convertidos.')
    _pausa_si_interactivo(args.no_pause)
    return rc


def _pausa_si_interactivo(no_pause):
    """Al doble-clic en Windows, evita que la consola se cierre de golpe."""
    if no_pause:
        return
    if os.name == 'nt' and sys.stdin and sys.stdin.isatty():
        try:
            input('\nPresiona ENTER para salir...')
        except (EOFError, KeyboardInterrupt):
            pass


if __name__ == '__main__':
    sys.exit(main())
