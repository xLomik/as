# -*- coding: utf-8 -*-
"""
Parser de reportes Bystronic "Lista de órdenes" (BySoft) en PDF.
Extrae los campos del PDF a un diccionario estructurado.

Estos PDF salen de la máquina láser con un layout fijo, así que el parseo
se basa en etiquetas conocidas (label-based), no en posiciones de píxeles.
"""
import re
import pdfplumber


def _num(s):
    """Convierte texto numérico español ('120,00', ',175', '51,94%') a float.
    Devuelve None si no hay número."""
    if s is None:
        return None
    s = str(s).strip()
    s = s.replace('%', '').replace('m²', '').replace('kg', '').replace('mm', '').strip()
    if s == '':
        return None
    # Caso ',175' -> '0,175'
    if s.startswith(','):
        s = '0' + s
    # Español: coma decimal, punto de miles. Aquí no hay miles, así que coma->punto.
    s = s.replace('.', '').replace(',', '.') if s.count(',') == 1 and s.count('.') <= 1 else s.replace(',', '.')
    try:
        return float(s)
    except ValueError:
        return None


def _after(line, label):
    """Devuelve lo que sigue a `label` en `line`, o None."""
    i = line.find(label)
    if i < 0:
        return None
    return line[i + len(label):].strip()


def parse_pdf(path):
    with pdfplumber.open(path) as pdf:
        pages = [p.extract_text() or '' for p in pdf.pages]
    text = '\n'.join(pages)
    lines = [ln.rstrip() for ln in text.split('\n')]
    L = lines  # alias

    d = {}

    def find(label):
        for ln in L:
            if label in ln:
                return ln
        return None

    # --- Cabecera ---
    d['titulo'] = L[0].strip() if L else 'Lista de órdenes'
    d['orden'] = L[1].strip() if len(L) > 1 else ''

    ln = find('JOB')
    if ln:
        m = re.search(r'JOB\s+(\S+)\s+Máquina\s+(.+)$', ln)
        if m:
            d['job'] = m.group(1).strip()
            d['maquina'] = m.group(2).strip()

    ln = find('Descripción')  # primera (cabecera)
    if ln:
        m = re.search(r'Descripción\s+(.+?)\s+Material\s+(.+)$', ln)
        if m:
            d['descripcion'] = m.group(1).strip()
            d['material'] = m.group(2).strip()

    ln = find('Grosor')
    if ln:
        m = re.search(r'Grosor\s+([\d.,]+)\s*mm', ln)
        if m:
            d['grosor'] = _num(m.group(1))

    ln = find('Cant. de piezas diferentes')
    if ln:
        m = re.search(r'Cant\. de piezas diferentes\s+(\d+)', ln)
        if m:
            d['cant_piezas_diferentes'] = int(m.group(1))

    # Tecnologías: la línea siguiente a "Tecnologías utilizadas"
    for i, x in enumerate(L):
        if 'Tecnologías utilizadas' in x and i + 1 < len(L):
            d['tecnologias'] = L[i + 1].strip()
            break

    ln = find('Gas de corte')
    if ln:
        m = re.search(r'Gas de corte\s+(\S+)', ln)
        if m:
            d['gas_corte'] = m.group(1).strip()

    ln = find('Desperdicio') and find('Objetivo de desperdicio')
    for x in L:
        if 'Desperdicio' in x and 'Objetivo' in x:
            m = re.search(r'Desperdicio\s+([\d.,]+)%', x)
            if m:
                d['desperdicio_pct'] = _num(m.group(1))
            break

    for x in L:
        if x.strip().startswith('Tiempo de corte') and 'Costes' in x:
            m = re.search(r'Tiempo de corte\s+([\d:]+)', x)
            if m:
                d['tiempo_corte_job'] = m.group(1)
            break

    for x in L:
        if 'Tiempos improd' in x:
            m = re.search(r'Tiempos improd\.?\s+([\d:]+)', x)
            if m:
                d['tiempos_improd_job'] = m.group(1)
            m2 = re.search(r'Peso total de desperdicio\s+([\d.,]+)\s*kg', x)
            if m2:
                d['peso_total_desperdicio'] = _num(m2.group(1))
            break

    for x in L:
        if x.strip().startswith('Tiempo total'):
            m = re.search(r'Tiempo total\s+([\d:]+)', x)
            if m:
                d['tiempo_total_job'] = m.group(1)
            break

    # --- Chapas ---
    chapas = []
    for i, x in enumerate(L):
        if x.strip() == 'Chapas':
            # buscar línea de datos tras el header
            for j in range(i + 1, min(i + 5, len(L))):
                m = re.search(r'([\d.,]+)\s*mm\s+([\d.,]+)\s*mm\s+([\d.,]+)\s*kg\s+(\d+)', L[j])
                if m:
                    chapas.append({
                        'x': _num(m.group(1)), 'y': _num(m.group(2)),
                        'peso': _num(m.group(3)), 'cantidad': int(m.group(4)),
                    })
            break
    d['chapas'] = chapas

    # --- Pieza (detalle) ---
    pieza = {}
    ln = find('Número de piezas')
    if ln:
        m = re.search(r'Número de piezas\s+(\d+)', ln)
        if m:
            pieza['numero_de_piezas'] = int(m.group(1))
    for x in L:
        if x.strip().startswith('Nombre') and 'Nominal' in x:
            m = re.search(r'Nombre\s+(\S+)\s+Nominal / Efectivo\s+(.+)$', x)
            if m:
                pieza['nombre'] = m.group(1)
                pieza['nominal_efectivo'] = m.group(2).strip()
            break
    for x in L:
        if x.strip().startswith('Descripción') and 'Dimensión X' in x:
            m = re.search(r'Descripción\s+(.+?)\s+Dimensión X\s+([\d.,]+)\s*mm', x)
            if m:
                pieza['descripcion'] = m.group(1).strip()
                pieza['dim_x'] = _num(m.group(2))
            break
    for x in L:
        if 'Dimensión Y' in x and 'Info1' in x:
            m = re.search(r'Dimensión Y\s+([\d.,]+)\s*mm', x)
            if m:
                pieza['dim_y'] = _num(m.group(1))
            break
    for x in L:
        if 'Superficie' in x:
            m = re.search(r'([\d.,]+)\s*m²', x)
            if m:
                pieza['superficie'] = _num(m.group(1))
            break
    for x in L:
        if x.strip().startswith('Info3') and 'Peso' in x or ('Peso' in x and 'kg' in x and 'total' not in x and 'Dimensión' not in x):
            m = re.search(r'Peso\s+([\d.,]+)\s*kg', x)
            if m:
                pieza['peso'] = _num(m.group(1))
                break
    for x in L:
        if 'Perimetro' in x or 'Perímetro' in x:
            m = re.search(r'Per[ií]metro\s+([\d.,]+)\s*mm', x)
            if m:
                pieza['perimetro'] = _num(m.group(1))
            break
    for x in L:
        if 'Tiempo de corte' in x and 'Tiempo total' in x and 'improd' not in x:
            m = re.search(r'Tiempo de corte\s+([\d:]+).*?(\d{2}:\d{2}:\d{2}).*?Tiempo total\s+([\d:]+)', x)
            if m:
                pieza['tiempo_corte'] = m.group(1)
                pieza['tiempos_improd'] = m.group(2)
                pieza['tiempo_total'] = m.group(3)
            break
    d['pieza'] = pieza

    # --- Planos de corte (pág 2) ---
    plano = {}
    for x in L:
        if 'Nombre de plano de corte' in x:
            m = re.search(r'Nombre de plano de corte\s+(\S+)\s+Dimensión X del plano\s+([\d.,]+)\s*mm', x)
            if m:
                plano['nombre'] = m.group(1)
                plano['dim_x_plano'] = _num(m.group(2))
            break
    for x in L:
        if x.strip().startswith('Pasadas'):
            m = re.search(r'Pasadas\s+(\d+)\s+Dimensión Y del plano\s+([\d.,]+)\s*mm', x)
            if m:
                plano['pasadas'] = int(m.group(1))
                plano['dim_y_plano'] = _num(m.group(2))
            break
    for x in L:
        if x.strip().startswith('Desperdicio') and 'Dimensión X de chapa' in x:
            m = re.search(r'Desperdicio\s+([\d.,]+)%\s+Dimensión X de chapa\s+([\d.,]+)\s*mm', x)
            if m:
                plano['desperdicio_pct'] = _num(m.group(1))
                plano['dim_x_chapa'] = _num(m.group(2))
            break
    for x in L:
        if x.strip().startswith('Número de piezas') and 'Dimensión Y de chapa' in x:
            m = re.search(r'Número de piezas\s+(\d+)\s+Dimensión Y de chapa\s+([\d.,]+)\s*mm', x)
            if m:
                plano['num_piezas'] = int(m.group(1))
                plano['dim_y_chapa'] = _num(m.group(2))
            break
    for x in L:
        if 'Tiempo de corte' in x and 'Tiempos improductivos' in x and 'Tiempo total' in x:
            m = re.search(r'Tiempo de corte\s+([\d:]+)\s+Tiempos improductivos\s+([\d:]+)\s+Tiempo total\s+([\d:]+)', x)
            if m:
                plano['tiempo_corte'] = m.group(1)
                plano['tiempos_improd'] = m.group(2)
                plano['tiempo_total'] = m.group(3)
            break
    d['plano'] = plano

    # --- Tabla de piezas (pág 2): N°. Nombre X Y Peso Tiempo Cantidad ---
    partes = []
    hdr = None
    for i, x in enumerate(L):
        if x.strip().startswith('N°.') and 'Nombre de pieza' in x:
            hdr = i
            break
    if hdr is not None:
        for j in range(hdr + 1, len(L)):
            x = L[j].strip()
            if x.startswith('FANALCA') or x == '' or x.startswith('Lista de'):
                break
            m = re.match(r'(\d+)\s+(\S+)\s+([\d.,]+)\s*mm\s+([\d.,]+)\s*mm\s+([\d.,]+)\s*kg\s+([\d:]+)\s+(\d+)', x)
            if m:
                partes.append({
                    'n': int(m.group(1)), 'nombre': m.group(2),
                    'dim_x': _num(m.group(3)), 'dim_y': _num(m.group(4)),
                    'peso': _num(m.group(5)), 'tiempo_corte': m.group(6),
                    'cantidad': int(m.group(7)),
                })
    d['partes'] = partes

    # --- Pie ---
    for x in L:
        if 'S.A.' in x or 'S.A ' in x:
            d['empresa'] = x.strip()
            break
    for i, x in enumerate(L):
        if 'Seite' in x or re.search(r'\d{2}\.\d{2}\.\d{4}', x):
            mu = re.match(r'(\S+)\s+(\d{2}\.\d{2}\.\d{4})', x.strip())
            if mu:
                d['usuario'] = mu.group(1)
                d['fecha'] = mu.group(2)
                break
    return d


if __name__ == '__main__':
    import json, sys
    p = sys.argv[1] if len(sys.argv) > 1 else '/tmp/up/82f26447-LP0626652.pdf'
    data = parse_pdf(p)
    print(json.dumps(data, ensure_ascii=False, indent=2))
