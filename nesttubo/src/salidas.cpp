// NestTubo - PDF, Excel y DXF escritos a mano (recetas de laboratorio/recetas y
// de la skill apps-windows-cpp). Sin Win32: se prueba nativo.
#include "salidas.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <map>

namespace nt {

namespace {

// Anchos de Helvetica y Helvetica-Bold (AFM de Adobe, milesimas del cuerpo) para
// los bytes 32..255 de WinAnsi. Generados de los AFM que trae matplotlib.
const double PI = 3.14159265358979323846;

const short ANCHO_HELV[224] = {278,278,355,556,556,889,667,191,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,278,278,584,584,584,556,1015,667,667,722,722,667,611,778,722,278,500,667,556,833,722,778,667,778,722,667,611,722,667,944,667,667,611,278,278,278,469,556,333,556,556,500,556,556,278,556,556,222,222,500,222,833,556,556,556,556,333,500,278,556,500,722,500,500,500,334,260,334,584,278,556,278,222,556,333,1000,556,556,333,1000,667,333,1000,278,611,278,278,222,222,333,333,350,556,1000,333,1000,500,333,944,278,500,667,278,333,556,556,556,556,260,556,333,737,370,556,584,333,737,333,400,584,333,333,333,556,537,278,333,333,365,556,834,834,834,611,667,667,667,667,667,667,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,500,556,556,556,556,278,278,278,278,556,556,556,556,556,556,556,584,611,556,556,556,556,500,556,500};
const short ANCHO_HELV_NEGRITA[224] = {278,333,474,556,556,889,722,238,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,333,333,584,584,584,611,975,722,722,722,722,667,611,778,722,278,556,722,611,833,722,778,667,778,722,667,611,722,667,944,667,667,611,333,278,333,584,556,333,556,611,556,611,556,333,611,611,278,278,556,278,889,611,611,611,611,389,556,333,611,556,778,556,556,500,389,280,389,584,278,556,278,278,556,500,1000,556,556,333,1000,667,333,1000,278,611,278,278,278,278,500,500,350,556,1000,333,1000,556,333,944,278,500,667,278,333,556,556,556,556,280,556,333,737,370,556,584,333,737,333,400,584,333,333,333,611,556,278,333,333,365,556,834,834,834,611,722,722,722,722,722,722,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,556,556,556,556,556,278,278,278,278,611,611,611,611,611,611,611,584,611,611,611,611,611,556,611,556};

// UTF-8 -> puntos de codigo
std::vector<uint32_t> codigos(const std::string& s) {
    std::vector<uint32_t> r;
    for (size_t i = 0; i < s.size();) {
        unsigned char c = (unsigned char)s[i];
        uint32_t cp;
        int n;
        if (c < 0x80) { cp = c; n = 1; }
        else if ((c >> 5) == 6) { cp = c & 0x1F; n = 2; }
        else if ((c >> 4) == 14) { cp = c & 0x0F; n = 3; }
        else if ((c >> 3) == 30) { cp = c & 0x07; n = 4; }
        else { r.push_back('?'); i++; continue; }
        if (i + n > s.size()) { r.push_back('?'); break; }
        bool ok = true;
        for (int k = 1; k < n; k++) {
            unsigned char d = (unsigned char)s[i + k];
            if ((d >> 6) != 2) ok = false;
            cp = (cp << 6) | (d & 0x3F);
        }
        r.push_back(ok ? cp : '?');
        i += ok ? n : 1;
    }
    return r;
}

std::string f1(double v) {   // numero con un decimal como mucho, punto decimal
    char b[64];
    if (std::fabs(v - std::round(v)) < 1e-9) std::snprintf(b, sizeof b, "%.0f", v);
    else std::snprintf(b, sizeof b, "%.1f", v);
    return b;
}

std::string f2(double v) {   // coordenadas del PDF
    char b[64];
    std::snprintf(b, sizeof b, "%.2f", v);
    return b;
}

std::string grupo_texto(const Grupo& g) {
    if (g.veces == 1) return "Barra " + std::to_string(g.primera + 1);
    return "Barras " + std::to_string(g.primera + 1) + "-" + std::to_string(g.primera + g.veces) + " (× " +
           std::to_string(g.veces) + ")";
}

}  // namespace

std::string a_winansi(const std::string& utf8) {
    static const std::map<uint32_t, unsigned char> especiales = {
        {0x20AC, 0x80}, {0x201A, 0x82}, {0x0192, 0x83}, {0x201E, 0x84}, {0x2026, 0x85}, {0x2020, 0x86},
        {0x2021, 0x87}, {0x02C6, 0x88}, {0x2030, 0x89}, {0x0160, 0x8A}, {0x2039, 0x8B}, {0x0152, 0x8C},
        {0x017D, 0x8E}, {0x2018, 0x91}, {0x2019, 0x92}, {0x201C, 0x93}, {0x201D, 0x94}, {0x2022, 0x95},
        {0x2013, 0x96}, {0x2014, 0x97}, {0x02DC, 0x98}, {0x2122, 0x99}, {0x0161, 0x9A}, {0x203A, 0x9B},
        {0x0153, 0x9C}, {0x017E, 0x9E}, {0x0178, 0x9F}};
    std::string r;
    for (uint32_t cp : codigos(utf8)) {
        if (cp < 0x80 || (cp >= 0xA0 && cp <= 0xFF)) r += (char)cp;
        else {
            auto it = especiales.find(cp);
            r += it == especiales.end() ? '?' : (char)it->second;
        }
    }
    return r;
}

std::string sin_tildes(const std::string& utf8) {
    // Latin-1 0xC0..0xFF a su letra base
    static const char* base = "AAAAAAACEEEEIIIIDNOOOOOxOUUUUYTsaaaaaaaceeeeiiiidnooooo/ouuuuyty";
    std::string r;
    for (uint32_t cp : codigos(utf8)) {
        if (cp >= 32 && cp < 127) r += (char)cp;
        else if (cp >= 0xC0 && cp <= 0xFF) r += base[cp - 0xC0];
        else if (cp == 0xB0 || cp == 0xBA) r += 'o';
        else if (cp == 0xAA) r += 'a';
        else if (cp == 0x2013 || cp == 0x2014) r += '-';
        else if (cp == '\t') r += ' ';
        else r += '?';
    }
    return r;
}

// ---------------------------------------------------------------------------
// PDF

namespace {

class Pdf {
public:
    static constexpr double ANCHO = 595, ALTO = 842, MARGEN = 40;

    void pagina() { paginas_.push_back(""); }
    double ancho_texto(const std::string& utf8, double tam, bool negrita) const {
        const short* t = negrita ? ANCHO_HELV_NEGRITA : ANCHO_HELV;
        double w = 0;
        for (unsigned char c : a_winansi(utf8)) w += c >= 32 ? t[c - 32] : 0;
        return w * tam / 1000.0;
    }
    // Recorta con "..." para que quepa en `max` puntos.
    std::string ajustar(const std::string& utf8, double tam, bool negrita, double max) const {
        if (ancho_texto(utf8, tam, negrita) <= max) return utf8;
        std::vector<uint32_t> cs = codigos(utf8);
        while (!cs.empty()) {
            cs.pop_back();
            std::string s;
            for (uint32_t c : cs) {   // volver a UTF-8
                if (c < 0x80) s += (char)c;
                else if (c < 0x800) { s += (char)(0xC0 | (c >> 6)); s += (char)(0x80 | (c & 0x3F)); }
                else { s += (char)(0xE0 | (c >> 12)); s += (char)(0x80 | ((c >> 6) & 0x3F)); s += (char)(0x80 | (c & 0x3F)); }
            }
            if (ancho_texto(s + "...", tam, negrita) <= max) return s + "...";
        }
        return "";
    }
    // alin: 0 izquierda, 1 derecha, 2 centro. y = linea base, medida desde arriba de la pagina.
    void texto(double x, double y, double tam, bool negrita, const std::string& utf8, int alin = 0, double gris = 0) {
        if (utf8.empty()) return;
        double w = ancho_texto(utf8, tam, negrita);
        if (alin == 1) x -= w;
        if (alin == 2) x -= w / 2;
        std::string lit = "(";
        for (unsigned char c : a_winansi(utf8)) {
            if (c == '(' || c == ')' || c == '\\') { lit += '\\'; lit += (char)c; }
            else if (c < 32 || c > 126) {
                char b[8];
                std::snprintf(b, sizeof b, "\\%03o", c);
                lit += b;
            } else lit += (char)c;
        }
        lit += ")";
        cuerpo() += f2(gris) + " g BT /" + std::string(negrita ? "F2 " : "F1 ") + f2(tam) + " Tf " + f2(x) + " " +
                    f2(ALTO - y) + " Td " + lit + " Tj ET\n";
    }
    // Rectangulo con esquina superior izquierda (x, y). relleno < 0: sin relleno.
    void rect(double x, double y, double w, double h, double relleno, bool borde, double grosor = 0.6) {
        std::string op = relleno >= 0 ? (borde ? "B" : "f") : "S";
        cuerpo() += (relleno >= 0 ? f2(relleno) + " g " : "") + f2(grosor) + " w 0 G " + f2(x) + " " + f2(ALTO - y - h) +
                    " " + f2(w) + " " + f2(h) + " re " + op + "\n";
    }
    void linea(double x1, double y1, double x2, double y2, double grosor = 0.5, double gris = 0) {
        cuerpo() += f2(grosor) + " w " + f2(gris) + " G " + f2(x1) + " " + f2(ALTO - y1) + " m " + f2(x2) + " " +
                    f2(ALTO - y2) + " l S\n";
    }
    std::string bytes() const {
        std::vector<std::string> objs;
        size_t n = paginas_.size();
        std::string kids;
        for (size_t i = 0; i < n; i++) kids += std::to_string(5 + 2 * i) + " 0 R ";
        objs.push_back("<< /Type /Catalog /Pages 2 0 R >>");
        objs.push_back("<< /Type /Pages /Kids [" + kids + "] /Count " + std::to_string(n) + " >>");
        objs.push_back("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
        objs.push_back("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>");
        for (size_t i = 0; i < n; i++) {
            objs.push_back("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents " +
                           std::to_string(6 + 2 * i) + " 0 R >>");
            objs.push_back("<< /Length " + std::to_string(paginas_[i].size()) + " >>\nstream\n" + paginas_[i] + "\nendstream");
        }
        std::string out = "%PDF-1.4\n%\xe2\xe3\xcf\xd3\n";
        std::vector<size_t> pos;
        for (size_t k = 0; k < objs.size(); k++) {
            pos.push_back(out.size());
            out += std::to_string(k + 1) + " 0 obj\n" + objs[k] + "\nendobj\n";
        }
        size_t xref = out.size();
        out += "xref\n0 " + std::to_string(objs.size() + 1) + "\n0000000000 65535 f \n";
        for (size_t p : pos) {
            char b[32];
            std::snprintf(b, sizeof b, "%010zu 00000 n \n", p);   // cada linea mide 20 bytes
            out += b;
        }
        out += "trailer\n<< /Size " + std::to_string(objs.size() + 1) + " /Root 1 0 R >>\nstartxref\n" +
               std::to_string(xref) + "\n%%EOF\n";
        return out;
    }
    int numero_pagina() const { return (int)paginas_.size(); }

private:
    std::string& cuerpo() {
        if (paginas_.empty()) paginas_.push_back("");
        return paginas_.back();
    }
    std::vector<std::string> paginas_;
};

// Tabla simple: columnas con ancho y alineacion.
struct Col {
    std::string titulo;
    double ancho;
    int alin;   // 0 izquierda, 1 derecha
};

double fila_tabla(Pdf& pdf, double x, double y, const std::vector<Col>& cols, const std::vector<std::string>& celdas,
                  double tam, bool negrita, double fondo = -1, int col_negrita = -1) {
    double alto = tam * 1.55;
    double total = 0;
    for (const auto& c : cols) total += c.ancho;
    if (fondo >= 0) pdf.rect(x, y, total, alto, fondo, false);
    double cx = x;
    for (size_t i = 0; i < cols.size() && i < celdas.size(); i++) {
        bool n = negrita || (int)i == col_negrita;
        std::string t = pdf.ajustar(celdas[i], tam, n, cols[i].ancho - 6);
        if (cols[i].alin == 1) pdf.texto(cx + cols[i].ancho - 3, y + tam * 1.12, tam, n, t, 1);
        else pdf.texto(cx + 3, y + tam * 1.12, tam, n, t, 0);
        cx += cols[i].ancho;
    }
    return y + alto;
}

void pie(Pdf& pdf, const std::string& trabajo, const std::string& fecha) {
    pdf.linea(Pdf::MARGEN, Pdf::ALTO - 30, Pdf::ANCHO - Pdf::MARGEN, Pdf::ALTO - 30, 0.4, 0.6);
    pdf.texto(Pdf::MARGEN, Pdf::ALTO - 18, 7.5, false, "NestTubo · " + trabajo + " · " + fecha, 0, 0.35);
    pdf.texto(Pdf::ANCHO - Pdf::MARGEN, Pdf::ALTO - 18, 7.5, false, "Página " + std::to_string(pdf.numero_pagina()), 1, 0.35);
}

// Dibujo de una barra a lo ancho de la pagina: despunte, piezas, zona muerta.
void dibujar_barra(Pdf& pdf, double x, double y, double w, double h, const ResultadoPerfil& r, const BarraPlan& b) {
    const Parametros& p = r.prob.param;
    const int e = r.prob.escala;
    double L = (double)p.largo_barra;
    double k = w / L;
    pdf.rect(x, y, w, h, 1.0, true, 0.6);
    if (p.despunte > 0) pdf.rect(x, y, p.despunte * k, h, 0.55, false);
    if (p.zona_muerta > 0) pdf.rect(x + (L - p.zona_muerta) * k, y, p.zona_muerta * k, h, 0.55, false);
    for (size_t i = 0; i < b.piezas.size(); i++) {
        const Colocada& c = b.piezas[i];
        double px = x + c.inicio * k, pw = c.largo * k;
        pdf.rect(px, y, pw, h, i % 2 ? 0.86 : 0.93, true, 0.4);
        std::string et = r.prob.nombres[c.id] + " " + medida(c.largo, e);
        if (pdf.ancho_texto(et, 7, false) > pw - 4) et = medida(c.largo, e);
        if (pdf.ancho_texto(et, 7, false) <= pw - 2) pdf.texto(px + pw / 2, y + h / 2 + 2.5, 7, false, et, 2);
    }
    pdf.rect(x, y, w, h, -1, true, 0.8);
}

}  // namespace

std::string informe_pdf(const std::vector<ResultadoPerfil>& rs, const std::string& trabajo, const std::string& fecha) {
    Pdf pdf;
    const double X = Pdf::MARGEN, AW = Pdf::ANCHO - 2 * Pdf::MARGEN, FONDO = Pdf::ALTO - 45;
    pdf.pagina();
    double y = 58;
    pdf.texto(X, y, 17, true, "Barras a enviar al proveedor de corte láser");
    y += 18;
    pdf.texto(X, y, 10, false, "Trabajo: " + trabajo + "      Fecha: " + fecha, 0, 0.25);
    y += 24;

    long long total = 0;
    for (const auto& r : rs)
        if (r.valido) total += r.barras_enviar();
    pdf.rect(X, y, AW, 40, 0.92, false);
    pdf.texto(X + 12, y + 26, 18, true, std::to_string(total) + (total == 1 ? " barra a enviar" : " barras a enviar"));
    pdf.texto(X + AW - 12, y + 25, 10, false,
              std::to_string(rs.size()) + (rs.size() == 1 ? " perfil" : " perfiles"), 1, 0.25);
    y += 54;

    std::vector<Col> cr = {{"Perfil", 115, 0}, {"Barra (mm)", 54, 1}, {"Barras a enviar", 70, 1}, {"Mínimo", 42, 1},
                           {"Margen", 40, 1},  {"Piezas", 40, 1},     {"Aprov.", 44, 1},          {"Mínimo demostrado", 110, 0}};
    std::vector<std::string> tit;
    for (const auto& c : cr) tit.push_back(c.titulo);
    y = fila_tabla(pdf, X, y, cr, tit, 8.5, true, 0.82);
    for (size_t i = 0; i < rs.size(); i++) {
        auto f = fila_resumen(rs[i]);
        f[5] = std::to_string(rs[i].plan.piezas_colocadas);   // la nota de las que no caben va abajo
        y = fila_tabla(pdf, X, y, cr, f, 8.5, false, i % 2 ? 0.96 : -1, 2);
    }
    pdf.linea(X, y, X + AW, y, 0.5, 0.5);
    y += 22;

    pdf.texto(X, y, 11, true, "Parámetros de la máquina por perfil (mm)");
    y += 8;
    std::vector<Col> cp = {{"Perfil", 140, 0}, {"Cara ancha", 65, 1}, {"Barra", 60, 1}, {"Despunte", 60, 1},
                           {"Zona muerta", 65, 1}, {"Separación", 65, 1}, {"Útil", 60, 1}};
    tit.clear();
    for (const auto& c : cp) tit.push_back(c.titulo);
    y = fila_tabla(pdf, X, y, cp, tit, 8.5, true, 0.82);
    for (size_t i = 0; i < rs.size(); i++) {
        const auto& pr = rs[i].prob;
        int e = pr.escala;
        y = fila_tabla(pdf, X, y, cp,
                       {pr.nombre, pr.cara ? medida(pr.cara, e) : "-", medida(pr.param.largo_barra, e), medida(pr.param.despunte, e),
                        medida(pr.param.zona_muerta, e), medida(pr.param.separacion, e), medida(pr.param.util(), e)},
                       8.5, false, i % 2 ? 0.96 : -1);
    }
    y += 18;

    // piezas que no caben y perfiles con error
    bool aviso = false;
    for (const auto& r : rs) {
        if (!r.valido) {
            if (!aviso) { pdf.texto(X, y, 11, true, "Avisos"); y += 15; aviso = true; }
            pdf.texto(X, y, 8.5, false, pdf.ajustar(r.prob.nombre + ": error en el cálculo (" + r.motivo + ")", 8.5, false, AW));
            y += 12;
            continue;
        }
        for (int id : r.plan.no_caben) {
            if (!aviso) { pdf.texto(X, y, 11, true, "Avisos"); y += 15; aviso = true; }
            const Pieza* pz = pieza_de(r.prob, id);
            i64 largo = pz ? pz->largo : 0, cant = pz ? pz->cantidad : 1;
            std::string que = cant > 1 ? "las " + std::to_string(cant) + " piezas \"" + r.prob.nombres[id] + "\" de " +
                                             medida(largo, r.prob.escala) + " mm no caben"
                                       : "la pieza \"" + r.prob.nombres[id] + "\" de " + medida(largo, r.prob.escala) + " mm no cabe";
            pdf.texto(X, y, 8.5, false,
                      pdf.ajustar(r.prob.nombre + ": " + que + " en el útil de " + medida(r.plan.parametros.util(), r.prob.escala) +
                                      " mm y " + (cant > 1 ? "quedaron" : "quedó") + " fuera del cálculo.",
                                  8.5, false, AW), 0, 0.15);
            y += 12;
        }
        if (r.plan.estado != Estado::completo) {
            if (!aviso) { pdf.texto(X, y, 11, true, "Avisos"); y += 15; aviso = true; }
            pdf.texto(X, y, 8.5, false, r.prob.nombre + ": el cálculo se cortó por tiempo o cancelación; es lo mejor encontrado.");
            y += 12;
        }
    }
    if (aviso) y += 8;
    const char* notas[] = {
        "Barras a enviar = mínimo calculado + margen de seguridad (el proveedor nestea con su propio programa).",
        "Mínimo demostrado: no existe ninguna forma de usar menos barras con estos datos.",
        "Cálculo solo por largos: cada pieza ocupa su largo de punta a punta; los ángulos no se encajan entre sí.",
        "Útil = barra - despunte - zona muerta. Entre piezas se deja la separación."};
    for (const char* n : notas) {
        if (y > FONDO - 12) break;
        pdf.texto(X, y, 7.5, false, n, 0, 0.35);
        y += 10;
    }
    pie(pdf, trabajo, fecha);

    // Un perfil por pagina (o mas, si no cabe)
    for (const auto& r : rs) {
        if (!r.valido) continue;
        const int e = r.prob.escala;
        auto cabecera = [&](bool sigue) {
            pdf.pagina();
            y = 58;
            pdf.texto(X, y, 15, true, r.prob.nombre + (sigue ? " (continuación)" : ""));
            y += 16;
            std::string sub = "Barra " + medida(r.prob.param.largo_barra, e) + " mm · útil " + medida(r.prob.param.util(), e) +
                              " mm · " + std::to_string(r.barras_enviar()) + " barras a enviar (" +
                              std::to_string(r.plan.barras.size()) + " + margen " + std::to_string(r.prob.margen) + ")";
            pdf.texto(X, y, 9.5, false, sub, 0, 0.25);
            y += 22;
        };
        cabecera(false);
        std::vector<Col> cpz = {{"Pieza", 200, 0}, {"Largo", 70, 1}, {"Inicio", 70, 1}, {"Fin", 70, 1}};
        for (const Grupo& g : agrupar(r.plan)) {
            const BarraPlan& b = r.plan.barras[g.primera];
            double necesita = 16 + 22 + 10 + 13 * (b.piezas.size() + 1) + 14;
            if (y + necesita > FONDO) {
                pie(pdf, trabajo, fecha);
                cabecera(true);
            }
            i64 fin = b.piezas.empty() ? 0 : b.piezas.back().fin;
            pdf.texto(X, y, 10.5, true, grupo_texto(g));
            pdf.texto(X + AW, y, 8.5, false,
                      std::to_string(b.piezas.size()) + " piezas · sobrante " + medida(r.prob.param.largo_barra - fin, e) + " mm",
                      1, 0.25);
            y += 6;
            dibujar_barra(pdf, X, y, AW, 20, r, b);
            y += 30;
            tit.clear();
            for (const auto& c : cpz) tit.push_back(c.titulo);
            y = fila_tabla(pdf, X, y, cpz, tit, 8, true, 0.88);
            for (size_t i = 0; i < b.piezas.size(); i++) {
                const Colocada& c = b.piezas[i];
                y = fila_tabla(pdf, X, y, cpz, {r.prob.nombres[c.id], medida(c.largo, e), medida(c.inicio, e), medida(c.fin, e)},
                               8, false, i % 2 ? 0.96 : -1);
            }
            y += 14;
        }
        for (int id : r.plan.no_caben) {
            if (y + 14 > FONDO) {
                pie(pdf, trabajo, fecha);
                cabecera(true);
            }
            const Pieza* pz = pieza_de(r.prob, id);
            pdf.texto(X, y, 9, true,
                      pdf.ajustar("No cabe: " + nombre_no_cabe(r.prob, id) + ", " + medida(pz ? pz->largo : 0, e) +
                                      " mm, más largo que el útil",
                                  9, true, AW));
            y += 14;
        }
        pie(pdf, trabajo, fecha);
    }
    return pdf.bytes();
}

// ---------------------------------------------------------------------------
// ZIP sin comprimir (metodo 0) con su CRC32: basta para un .xlsx.

namespace {
uint32_t crc32(const std::string& d) {
    static uint32_t tabla[256];
    static bool lista = false;
    if (!lista) {
        for (uint32_t i = 0; i < 256; i++) {
            uint32_t c = i;
            for (int k = 0; k < 8; k++) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
            tabla[i] = c;
        }
        lista = true;
    }
    uint32_t c = 0xFFFFFFFFu;
    for (unsigned char b : d) c = tabla[(c ^ b) & 0xFF] ^ (c >> 8);
    return c ^ 0xFFFFFFFFu;
}
void u16(std::string& s, uint32_t v) { s += (char)(v & 0xFF); s += (char)((v >> 8) & 0xFF); }
void u32(std::string& s, uint32_t v) { u16(s, v & 0xFFFF); u16(s, v >> 16); }
}  // namespace

std::string zip_sin_comprimir(const std::vector<std::pair<std::string, std::string>>& archivos) {
    std::string out, central;
    const uint32_t hora = (12u << 11), fecha = ((2026u - 1980u) << 9) | (1u << 5) | 1u;   // fija: el archivo es determinista
    for (const auto& a : archivos) {
        uint32_t crc = crc32(a.second), off = (uint32_t)out.size(), n = (uint32_t)a.second.size();
        u32(out, 0x04034b50); u16(out, 20); u16(out, 0x0800); u16(out, 0); u16(out, hora); u16(out, fecha);
        u32(out, crc); u32(out, n); u32(out, n); u16(out, (uint32_t)a.first.size()); u16(out, 0);
        out += a.first + a.second;
        u32(central, 0x02014b50); u16(central, 20); u16(central, 20); u16(central, 0x0800); u16(central, 0);
        u16(central, hora); u16(central, fecha); u32(central, crc); u32(central, n); u32(central, n);
        u16(central, (uint32_t)a.first.size()); u16(central, 0); u16(central, 0); u16(central, 0); u16(central, 0);
        u32(central, 0); u32(central, off);
        central += a.first;
    }
    uint32_t inicio = (uint32_t)out.size();
    out += central;
    u32(out, 0x06054b50); u16(out, 0); u16(out, 0); u16(out, (uint32_t)archivos.size()); u16(out, (uint32_t)archivos.size());
    u32(out, (uint32_t)central.size()); u32(out, inicio); u16(out, 0);
    return out;
}

// ---------------------------------------------------------------------------
// Excel

namespace {

std::string xml(const std::string& s) {
    std::string r;
    for (char c : s) {
        switch (c) {
        case '&': r += "&amp;"; break;
        case '<': r += "&lt;"; break;
        case '>': r += "&gt;"; break;
        case '"': r += "&quot;"; break;
        default:
            if ((unsigned char)c >= 32 || c == '\t' || c == '\n') r += c;
        }
    }
    return r;
}

std::string col_letra(int c) {
    std::string s;
    for (c++; c > 0; c = (c - 1) / 26) s = (char)('A' + (c - 1) % 26) + s;
    return s;
}

// Una celda: texto o numero. estilo: 0 normal, 1 negrita, 2 porcentaje, 3 numero en negrita, 4 titulo (negrita, fondo gris)
struct Celda {
    bool numero = false;
    double valor = 0;
    std::string texto;
    int estilo = 0;
};
Celda T(const std::string& t, int estilo = 0) { Celda c; c.texto = t; c.estilo = estilo; return c; }
Celda N(double v, int estilo = 0) { Celda c; c.numero = true; c.valor = v; c.estilo = estilo; return c; }
Celda V() { return Celda{}; }

std::string hoja(const std::vector<std::vector<Celda>>& filas, const std::vector<double>& anchos, bool horizontal) {
    std::string s = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
                    "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">"
                    "<sheetPr><pageSetUpPr fitToPage=\"1\"/></sheetPr>"
                    "<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>"
                    "<cols>";
    for (size_t i = 0; i < anchos.size(); i++)
        s += "<col min=\"" + std::to_string(i + 1) + "\" max=\"" + std::to_string(i + 1) + "\" width=\"" + f1(anchos[i]) +
             "\" customWidth=\"1\"/>";
    s += "</cols><sheetData>";
    for (size_t f = 0; f < filas.size(); f++) {
        s += "<row r=\"" + std::to_string(f + 1) + "\">";
        for (size_t c = 0; c < filas[f].size(); c++) {
            const Celda& x = filas[f][c];
            if (!x.numero && x.texto.empty()) continue;
            std::string ref = col_letra((int)c) + std::to_string(f + 1);
            std::string st = x.estilo ? " s=\"" + std::to_string(x.estilo) + "\"" : "";
            if (x.numero) {
                char b[64];
                std::snprintf(b, sizeof b, "%.10g", x.valor);
                s += "<c r=\"" + ref + "\"" + st + "><v>" + b + "</v></c>";
            } else {
                s += "<c r=\"" + ref + "\"" + st + " t=\"inlineStr\"><is><t xml:space=\"preserve\">" + xml(x.texto) + "</t></is></c>";
            }
        }
        s += "</row>";
    }
    s += "</sheetData><pageMargins left=\"0.5\" right=\"0.5\" top=\"0.6\" bottom=\"0.6\" header=\"0.3\" footer=\"0.3\"/>"
         "<pageSetup paperSize=\"9\" orientation=\"" +
         std::string(horizontal ? "landscape" : "portrait") + "\" fitToWidth=\"1\" fitToHeight=\"0\"/></worksheet>";
    return s;
}

}  // namespace

std::string informe_xlsx(const std::vector<ResultadoPerfil>& rs, const std::string& trabajo, const std::string& fecha) {
    // Resumen
    std::vector<std::vector<Celda>> res;
    res.push_back({T("Perfil", 4), T("Barra (mm)", 4), T("Barras a enviar", 4), T("Mínimo calculado", 4), T("Margen", 4),
                   T("Piezas", 4), T("No caben", 4), T("Aprovechamiento", 4), T("Mínimo demostrado", 4), T("Despunte", 4),
                   T("Zona muerta", 4), T("Separación", 4), T("Útil", 4), T("Cara ancha", 4)});
    long long total = 0;
    for (const auto& r : rs) {
        const auto& p = r.prob;
        double e = p.escala;
        if (!r.valido) {
            res.push_back({T(p.nombre), N(p.param.largo_barra / e), T("ERROR"), V(), V(), V(), V(), V(), T(r.motivo)});
            continue;
        }
        total += r.barras_enviar();
        std::string dem = r.plan.demostrado ? "sí" : "no: podrían sobrar hasta " + std::to_string(r.plan.sobran_hasta());
        if (r.plan.estado != Estado::completo) dem += " (cortado por tiempo o cancelación)";
        res.push_back({T(p.nombre), N(p.param.largo_barra / e), N((double)r.barras_enviar(), 3), N((double)r.plan.barras.size()),
                       N(p.margen), N((double)r.plan.piezas_colocadas), N((double)r.plan.piezas_no_caben),
                       N(r.plan.aprovechamiento(), 2), T(dem), N(p.param.despunte / e), N(p.param.zona_muerta / e),
                       N(p.param.separacion / e), N(p.param.util() / e), p.cara ? N(p.cara / e) : V()});
    }
    res.push_back({});
    res.push_back({T("Total barras a enviar", 1), V(), N((double)total, 3)});
    res.push_back({T("Trabajo: " + trabajo + " · " + fecha)});

    // Plan: una fila por pieza; perfil y barra en todas para poder filtrar
    std::vector<std::vector<Celda>> plan;
    plan.push_back({T("Perfil", 4), T("Barras", 4), T("Veces", 4), T("Pieza", 4), T("Largo (mm)", 4), T("Inicio (mm)", 4),
                    T("Fin (mm)", 4), T("Sobrante de la barra (mm)", 4)});
    for (const auto& r : rs) {
        if (!r.valido) continue;
        double e = r.prob.escala;
        for (const Grupo& g : agrupar(r.plan)) {
            const BarraPlan& b = r.plan.barras[g.primera];
            std::string barras = g.veces == 1 ? std::to_string(g.primera + 1)
                                              : std::to_string(g.primera + 1) + "-" + std::to_string(g.primera + g.veces);
            i64 fin = b.piezas.empty() ? 0 : b.piezas.back().fin;
            for (size_t i = 0; i < b.piezas.size(); i++) {
                const Colocada& c = b.piezas[i];
                plan.push_back({T(r.prob.nombre), T(barras), N(g.veces), T(r.prob.nombres[c.id]), N(c.largo / e), N(c.inicio / e),
                                N(c.fin / e), i == 0 ? N((r.prob.param.largo_barra - fin) / e) : V()});
            }
        }
        for (int id : r.plan.no_caben) {
            const Pieza* pz = pieza_de(r.prob, id);
            plan.push_back({T(r.prob.nombre), T("NO CABE"), V(), T(nombre_no_cabe(r.prob, id)), N((pz ? pz->largo : 0) / e), V(), V(),
                            V()});
        }
    }

    // Piezas: lo digitado
    std::vector<std::vector<Celda>> pzs;
    pzs.push_back({T("Perfil", 4), T("Pieza", 4), T("Largo (mm)", 4), T("Ángulo 1", 4), T("Ángulo 2", 4), T("Cantidad", 4)});
    for (const auto& r : rs) {
        double e = r.prob.escala;
        for (const auto& pz : r.prob.piezas) {
            double a1 = pz.id < (int)r.prob.angulo1.size() ? r.prob.angulo1[pz.id] / 10.0 : 0;
            double a2 = pz.id < (int)r.prob.angulo2.size() ? r.prob.angulo2[pz.id] / 10.0 : 0;
            pzs.push_back({T(r.prob.nombre), T(r.prob.nombres[pz.id]), N(pz.largo / e), N(a1), N(a2), N(pz.cantidad)});
        }
    }

    const std::string ct =
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">"
        "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>"
        "<Default Extension=\"xml\" ContentType=\"application/xml\"/>"
        "<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
        "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>"
        "<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        "<Override PartName=\"/xl/worksheets/sheet2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        "<Override PartName=\"/xl/worksheets/sheet3.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        "</Types>";
    const std::string rels =
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>"
        "</Relationships>";
    const std::string wb =
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
        "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" "
        "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>"
        "<sheet name=\"Resumen\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Plan\" sheetId=\"2\" r:id=\"rId2\"/>"
        "<sheet name=\"Piezas\" sheetId=\"3\" r:id=\"rId3\"/></sheets></workbook>";
    const std::string wbrels =
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>"
        "<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet2.xml\"/>"
        "<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet3.xml\"/>"
        "<Relationship Id=\"rId4\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>"
        "</Relationships>";
    const std::string estilos =
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"
        "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">"
        "<numFmts count=\"1\"><numFmt numFmtId=\"164\" formatCode=\"0.0%\"/></numFmts>"
        "<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font><font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>"
        "<fills count=\"3\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill>"
        "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FFDDDDDD\"/><bgColor indexed=\"64\"/></patternFill></fill></fills>"
        "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"
        "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>"
        "<cellXfs count=\"5\">"
        "<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>"
        "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>"
        "<xf numFmtId=\"164\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/>"
        "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>"
        "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/>"
        "</cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>";
    return zip_sin_comprimir({
        {"[Content_Types].xml", ct},
        {"_rels/.rels", rels},
        {"xl/workbook.xml", wb},
        {"xl/_rels/workbook.xml.rels", wbrels},
        {"xl/styles.xml", estilos},
        {"xl/worksheets/sheet1.xml", hoja(res, {24, 11, 15, 16, 9, 9, 10, 16, 30, 10, 12, 11, 9, 11}, true)},
        {"xl/worksheets/sheet2.xml", hoja(plan, {24, 10, 8, 26, 12, 12, 12, 24}, false)},
        {"xl/worksheets/sheet3.xml", hoja(pzs, {24, 26, 12, 10, 10, 10}, false)},
    });
}

// ---------------------------------------------------------------------------
// DXF (R12 ASCII: solo LINE y TEXT, capas BARRA, PIEZAS, TEXTO y ZONA_MUERTA)

namespace {

struct Dxf {
    std::string s;
    void linea(double x1, double y1, double x2, double y2, const char* capa) {
        s += "0\r\nLINE\r\n8\r\n" + std::string(capa) + "\r\n10\r\n" + num(x1) + "\r\n20\r\n" + num(y1) + "\r\n30\r\n0.0\r\n11\r\n" +
             num(x2) + "\r\n21\r\n" + num(y2) + "\r\n31\r\n0.0\r\n";
    }
    void texto(double x, double y, double alto, const std::string& t, const char* capa) {
        s += "0\r\nTEXT\r\n8\r\n" + std::string(capa) + "\r\n10\r\n" + num(x) + "\r\n20\r\n" + num(y) + "\r\n30\r\n0.0\r\n40\r\n" +
             num(alto) + "\r\n1\r\n" + sin_tildes(t) + "\r\n";
    }
    static std::string num(double v) {
        char b[64];
        std::snprintf(b, sizeof b, "%.3f", v);
        return b;
    }
    std::string fin(double xmax, double ymin, double ymax) const {
        return "0\r\nSECTION\r\n2\r\nHEADER\r\n9\r\n$ACADVER\r\n1\r\nAC1009\r\n9\r\n$EXTMIN\r\n10\r\n0.0\r\n20\r\n" + num(ymin) +
               "\r\n30\r\n0.0\r\n9\r\n$EXTMAX\r\n10\r\n" + num(xmax) + "\r\n20\r\n" + num(ymax) +
               "\r\n30\r\n0.0\r\n0\r\nENDSEC\r\n0\r\nSECTION\r\n2\r\nENTITIES\r\n" + s + "0\r\nENDSEC\r\n0\r\nEOF\r\n";
    }
};

std::string nombre_archivo(const std::string& s) {
    std::string r;
    for (char c : sin_tildes(s)) {
        if (c == '<' || c == '>' || c == ':' || c == '"' || c == '/' || c == '\\' || c == '|' || c == '?' || c == '*') r += '-';
        else r += c;
    }
    while (!r.empty() && (r.back() == ' ' || r.back() == '.')) r.pop_back();
    return r.empty() ? "perfil" : r;
}

}  // namespace

std::vector<ArchivoDxf> dxf_perfil(const ResultadoPerfil& r) {
    std::vector<ArchivoDxf> out;
    if (!r.valido) return out;
    const double e = r.prob.escala;
    const Parametros& p = r.prob.param;
    const double L = p.largo_barra / e;
    const bool sin_cara = r.prob.cara <= 0;
    const double cara = sin_cara ? CARA_SIN_DATO : r.prob.cara / e;
    const double alto_texto = std::max(4.0, std::min(cara * 0.28, 25.0));
    for (const Grupo& g : agrupar(r.plan)) {
        const BarraPlan& b = r.plan.barras[g.primera];
        Dxf d;
        // la barra y la zona muerta
        d.linea(0, 0, L, 0, "BARRA");
        d.linea(L, 0, L, cara, "BARRA");
        d.linea(L, cara, 0, cara, "BARRA");
        d.linea(0, cara, 0, 0, "BARRA");
        if (p.despunte > 0) d.linea(p.despunte / e, 0, p.despunte / e, cara, "BARRA");
        if (p.zona_muerta > 0) {
            double zx = L - p.zona_muerta / e;
            d.linea(zx, 0, zx, cara, "ZONA_MUERTA");
            d.linea(zx, 0, L, cara, "ZONA_MUERTA");
            d.linea(zx, cara, L, 0, "ZONA_MUERTA");
        }
        // cada pieza vista por la cara mas ancha: abajo su largo de punta a punta; arriba,
        // cada extremo en angulo retrocede cara * tan(angulo) (0 = recto). Trapecio, como un marco.
        int fila_etiqueta = 0;
        for (const Colocada& c : b.piezas) {
            double x0 = c.inicio / e, lg = c.largo / e;
            double a1 = c.id < (int)r.prob.angulo1.size() ? r.prob.angulo1[c.id] / 10.0 : 0;
            double a2 = c.id < (int)r.prob.angulo2.size() ? r.prob.angulo2[c.id] / 10.0 : 0;
            double r1 = cara * std::tan(a1 * PI / 180.0), r2 = cara * std::tan(a2 * PI / 180.0);
            if (r1 + r2 > lg) {   // extremos que se cruzan: se dibujan hasta el centro
                double k = lg / (r1 + r2);
                r1 *= k;
                r2 *= k;
            }
            d.linea(x0, 0, x0 + lg, 0, "PIEZAS");
            d.linea(x0 + lg, 0, x0 + lg - r2, cara, "PIEZAS");
            d.linea(x0 + lg - r2, cara, x0 + r1, cara, "PIEZAS");
            d.linea(x0 + r1, cara, x0, 0, "PIEZAS");
            std::string et = r.prob.nombres[c.id] + " " + f1(lg);
            double ancho_et = sin_tildes(et).size() * alto_texto * 0.75;
            if (ancho_et < lg - r1 - r2 - 1.5 * alto_texto) {   // dentro, despues del extremo en angulo
                d.texto(x0 + r1 + alto_texto * 0.75, cara / 2 - alto_texto / 2, alto_texto, et, "TEXTO");
            } else {   // no cabe dentro: debajo de la barra, en dos alturas para que no se pisen
                d.texto(x0, -alto_texto * (1.6 + 1.5 * (fila_etiqueta++ % 2)), alto_texto * 0.8, et, "TEXTO");
            }
        }
        i64 fin = b.piezas.empty() ? 0 : b.piezas.back().fin;
        std::string titulo = r.prob.nombre + " - " + sin_tildes(grupo_texto(g)) + " - barra " + f1(L) + " - sobrante " +
                             f1((p.largo_barra - fin) / e) + " mm";
        if (sin_cara) titulo += " - cara no indicada: dibujada de " + std::to_string(CARA_SIN_DATO) + " mm";
        d.texto(0, cara + alto_texto * 1.2, alto_texto * 1.2, titulo, "TEXTO");
        std::string nombre = nombre_archivo(r.prob.nombre) + " - ";
        if (g.veces == 1) nombre += "barra " + std::to_string(g.primera + 1);
        else nombre += "barras " + std::to_string(g.primera + 1) + "-" + std::to_string(g.primera + g.veces) + " x" +
                       std::to_string(g.veces);
        out.push_back({nombre + ".dxf", d.fin(L, -alto_texto * 5, cara + alto_texto * 3)});
    }
    return out;
}

}  // namespace nt
