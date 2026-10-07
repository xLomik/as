// NestTubo - lectura del trabajo digitado y textos del resultado (sin Win32).
#include "trabajo.h"

#include <algorithm>
#include <cstdio>
#include <map>
#include <sstream>

namespace nt {

namespace {

std::string recortar(const std::string& s) {
    size_t a = 0, b = s.size();
    while (a < b && (s[a] == ' ' || s[a] == '\t')) a++;
    while (b > a && (s[b - 1] == ' ' || s[b - 1] == '\t')) b--;
    return s.substr(a, b - a);
}

std::string minusculas(const std::string& s) {
    std::string r = recortar(s);
    for (char& c : r)
        if (c >= 'A' && c <= 'Z') c = (char)(c - 'A' + 'a');
    return r;
}

std::string miles(long long v) {   // 12345 -> "12.345"
    std::string s = std::to_string(v < 0 ? -v : v), r;
    for (size_t i = 0; i < s.size(); i++) {
        if (i && (s.size() - i) % 3 == 0) r += '.';
        r += s[i];
    }
    return v < 0 ? "-" + r : r;
}

std::string fila_txt(size_t i) { return "fila " + std::to_string(i + 1); }

}  // namespace

PerfilTxt perfil_nuevo(const std::string& nombre) {
    return PerfilTxt{nombre, "", BARRA_INICIAL, DESPUNTE_INICIAL, ZONA_MUERTA_INICIAL, SEPARACION_INICIAL, MARGEN_INICIAL};
}

bool fila_vacia(const PiezaTxt& p) {
    return recortar(p.perfil).empty() && recortar(p.nombre).empty() && recortar(p.largo).empty() &&
           recortar(p.angulo1).empty() && recortar(p.angulo2).empty() && recortar(p.cantidad).empty();
}

bool fila_vacia(const PerfilTxt& p) {
    return recortar(p.nombre).empty() && recortar(p.cara).empty() && recortar(p.barra).empty() &&
           recortar(p.despunte).empty() && recortar(p.zona_muerta).empty() && recortar(p.separacion).empty() &&
           recortar(p.margen).empty();
}

std::string medida(i64 v, int escala) {
    if (escala == 1) return std::to_string(v);
    std::string s = std::to_string(v / 10);
    i64 d = v % 10;
    if (d < 0) d = -d;
    return d ? s + "," + std::to_string(d) : s;
}

bool preparar(const Trabajo& t, std::vector<ProblemaPerfil>& problemas, std::vector<std::string>& errores) {
    problemas.clear();
    errores.clear();
    auto numero = [&](const std::string& texto, const std::string& donde, const std::string& que, bool positivo,
                      i64& dec) {
        if (!leer_decimas(texto, dec)) {
            errores.push_back(donde + ": " + que + " no es un número (\"" + recortar(texto) + "\")");
            return false;
        }
        if (positivo && dec <= 0) {
            errores.push_back(donde + ": " + que + " debe ser mayor que cero");
            return false;
        }
        return true;
    };
    auto entero = [&](const std::string& texto, const std::string& donde, const std::string& que, long long minimo,
                      long long maximo, long long& v) {
        std::string s = recortar(texto);
        bool ok = !s.empty() && s.size() <= 9 && std::all_of(s.begin(), s.end(), [](char c) { return c >= '0' && c <= '9'; });
        if (ok) v = std::stoll(s);
        if (!ok || v < minimo || v > maximo) {
            errores.push_back(donde + ": " + que + " debe ser un entero entre " + std::to_string(minimo) + " y " +
                              miles(maximo) + " (\"" + s + "\")");
            return false;
        }
        return true;
    };

    // Perfiles, en decimas
    struct PerfilDec {
        std::string nombre;
        i64 cara = 0, barra = 0, despunte = 0, zona = 0, sep = 0;
        int margen = 0;
        bool ok = true;
    };
    std::vector<PerfilDec> perfiles;
    std::map<std::string, int> indice;
    for (size_t i = 0; i < t.perfiles.size(); i++) {
        const PerfilTxt& p = t.perfiles[i];
        if (fila_vacia(p)) continue;
        std::string donde = "Perfiles, " + fila_txt(i);
        PerfilDec d;
        d.nombre = recortar(p.nombre);
        if (d.nombre.empty()) {
            errores.push_back(donde + ": falta el nombre del perfil");
            d.ok = false;
        } else if (indice.count(minusculas(d.nombre))) {
            errores.push_back(donde + ": el perfil \"" + d.nombre + "\" está repetido");
            d.ok = false;
        }
        if (!recortar(p.cara).empty()) d.ok &= numero(p.cara, donde, "la cara más ancha", true, d.cara);
        d.ok &= numero(p.barra, donde, "el largo de barra", true, d.barra);
        d.ok &= numero(p.despunte, donde, "el despunte", false, d.despunte);
        d.ok &= numero(p.zona_muerta, donde, "la zona muerta", false, d.zona);
        d.ok &= numero(p.separacion, donde, "la separación", false, d.sep);
        long long margen = 0;
        if (!recortar(p.margen).empty()) d.ok &= entero(p.margen, donde, "el margen", 0, 1000, margen);
        d.margen = (int)margen;
        if (d.ok && d.barra - d.despunte - d.zona <= 0) {
            errores.push_back(donde + ": despunte y zona muerta se comen toda la barra");
            d.ok = false;
        }
        if (!d.nombre.empty() && !indice.count(minusculas(d.nombre))) indice[minusculas(d.nombre)] = (int)perfiles.size();
        perfiles.push_back(d);
    }

    // Piezas, en decimas, agrupadas por perfil
    struct PiezaDec {
        int id;
        i64 largo;
        int cantidad;
    };
    std::vector<std::vector<PiezaDec>> por_perfil(perfiles.size());
    std::vector<std::string> nombres(t.piezas.size());
    for (size_t i = 0; i < t.piezas.size(); i++) {
        const PiezaTxt& p = t.piezas[i];
        if (fila_vacia(p)) continue;
        std::string donde = "Piezas, " + fila_txt(i);
        nombres[i] = recortar(p.nombre).empty() ? "Fila " + std::to_string(i + 1) : recortar(p.nombre);
        bool ok = true;
        auto it = indice.find(minusculas(p.perfil));
        if (recortar(p.perfil).empty()) {
            errores.push_back(donde + ": falta el perfil");
            ok = false;
        } else if (it == indice.end()) {
            errores.push_back(donde + ": el perfil \"" + recortar(p.perfil) + "\" no está en la tabla de perfiles");
            ok = false;
        }
        i64 largo = 0, a1 = 0, a2 = 0;
        ok &= numero(p.largo, donde, "el largo", true, largo);
        for (const std::string* a : {&p.angulo1, &p.angulo2}) {
            i64& v = a == &p.angulo1 ? a1 : a2;
            const char* que = a == &p.angulo1 ? "el ángulo 1" : "el ángulo 2";
            if (recortar(*a).empty()) continue;
            if (numero(*a, donde, que, false, v) && v >= 900) {
                errores.push_back(donde + ": " + que + " debe ser menor que 90 (0 = corte recto)");
                ok = false;
            }
        }
        long long cantidad = 0;
        ok &= entero(p.cantidad, donde, "la cantidad", 1, 100000, cantidad);
        if (ok) por_perfil[it->second].push_back({(int)i, largo, (int)cantidad});
    }
    if (!errores.empty()) return false;

    for (size_t k = 0; k < perfiles.size(); k++) {
        const PerfilDec& d = perfiles[k];
        if (por_perfil[k].empty()) continue;   // perfil sin piezas: no se calcula
        // mm si todo es entero; decimas si algun dato trae decimal
        bool decimal = d.barra % 10 || d.despunte % 10 || d.zona % 10 || d.sep % 10;
        for (const auto& pz : por_perfil[k]) decimal |= (pz.largo % 10 != 0);
        int div = decimal ? 1 : 10;
        ProblemaPerfil pr;
        pr.nombre = d.nombre;
        pr.escala = decimal ? 10 : 1;
        pr.param = Parametros{d.barra / div, d.despunte / div, d.zona / div, d.sep / div};
        pr.cara = d.cara / div;   // la cara solo se dibuja: si trae decimal en mm se redondea abajo
        pr.margen = d.margen;
        pr.nombres = nombres;
        for (const auto& pz : por_perfil[k]) pr.piezas.push_back({pz.id, pz.largo / div, pz.cantidad});
        problemas.push_back(std::move(pr));
    }
    if (problemas.empty()) {
        errores.push_back("No hay piezas para calcular");
        return false;
    }
    return true;
}

// ---------------------------------------------------------------------------
// Textos del resultado

std::vector<std::string> columnas_resumen() {
    return {"Perfil", "Barra", "Barras a enviar", "Mínimo", "Margen", "Piezas", "Aprov.", "Mínimo demostrado"};
}

std::vector<std::string> fila_resumen(const ResultadoPerfil& r) {
    const Plan& p = r.plan;
    if (!r.valido) return {r.prob.nombre, medida(r.prob.param.largo_barra, r.prob.escala), "ERROR", "", "", "", "", r.motivo};
    char apr[32];
    std::snprintf(apr, sizeof apr, "%.1f %%", 100.0 * p.aprovechamiento());
    for (char* c = apr; *c; c++)
        if (*c == '.') *c = ',';
    std::string dem;
    if (p.demostrado) dem = "sí";
    else dem = "no: podrían sobrar hasta " + std::to_string(p.sobran_hasta());
    if (p.estado == Estado::tiempo_agotado) dem += " (tiempo agotado)";
    if (p.estado == Estado::cancelado) dem += " (cancelado)";
    std::string piezas = miles(p.piezas_colocadas);
    if (!p.no_caben.empty()) piezas += " (+" + std::to_string(p.no_caben.size()) + " no caben)";
    return {r.prob.nombre,
            medida(r.prob.param.largo_barra, r.prob.escala),
            miles(r.barras_enviar()),
            miles((long long)p.barras.size()),
            std::to_string(r.prob.margen),
            piezas,
            apr,
            dem};
}

std::vector<std::string> columnas_plan() { return {"Barra", "Pieza", "Largo", "Inicio", "Fin", "Sobrante"}; }

std::vector<std::vector<std::string>> filas_plan(const ResultadoPerfil& r) {
    // Una fila por pieza. La primera pieza de cada grupo de barras iguales lleva
    // el numero de barra ("3" o "1-5 (× 5)") y el sobrante (lo que queda de la
    // barra despues de la ultima pieza, zona muerta incluida).
    std::vector<std::vector<std::string>> filas;
    if (!r.valido) return filas;
    const Plan& p = r.plan;
    const int e = r.prob.escala;
    for (const Grupo& g : agrupar(p)) {
        const BarraPlan& b = p.barras[g.primera];
        std::string barra = g.veces == 1 ? std::to_string(g.primera + 1)
                                         : std::to_string(g.primera + 1) + "-" + std::to_string(g.primera + g.veces) +
                                               " (× " + std::to_string(g.veces) + ")";
        i64 fin = b.piezas.empty() ? 0 : b.piezas.back().fin;
        for (size_t i = 0; i < b.piezas.size(); i++) {
            const Colocada& c = b.piezas[i];
            filas.push_back({i == 0 ? barra : "", r.prob.nombres[c.id], medida(c.largo, e), medida(c.inicio, e),
                             medida(c.fin, e), i == 0 ? medida(p.parametros.largo_barra - fin, e) : ""});
        }
    }
    for (int id : p.no_caben) {
        i64 largo = 0;
        for (const Pieza& pz : r.prob.piezas)
            if (pz.id == id) largo = pz.largo;
        filas.push_back({"NO CABE", r.prob.nombres[id], medida(largo, e), "", "",
                         "más largo que el útil (" + medida(p.parametros.util(), e) + ")"});
    }
    return filas;
}

// ---------------------------------------------------------------------------
// Archivo de trabajo
//
//   NESTTUBO<TAB>1
//   [perfiles]
//   nombre<TAB>cara<TAB>barra<TAB>despunte<TAB>zona_muerta<TAB>separacion<TAB>margen
//   [piezas]
//   perfil<TAB>nombre<TAB>largo<TAB>angulo1<TAB>angulo2<TAB>cantidad
//
// Campos que falten al final de una fila (archivos viejos) se rellenan: los de
// perfil con los valores iniciales, los de pieza en blanco.

namespace {
std::string limpio(const std::string& s) {
    std::string r = s;
    for (char& c : r)
        if (c == '\t' || c == '\r' || c == '\n') c = ' ';
    return r;
}
}  // namespace

std::string a_texto(const Trabajo& t) {
    std::ostringstream o;
    o << "NESTTUBO\t1\r\n[perfiles]\r\n";
    for (const auto& p : t.perfiles) {
        if (fila_vacia(p)) continue;
        o << limpio(p.nombre) << '\t' << limpio(p.cara) << '\t' << limpio(p.barra) << '\t' << limpio(p.despunte) << '\t'
          << limpio(p.zona_muerta) << '\t' << limpio(p.separacion) << '\t' << limpio(p.margen) << "\r\n";
    }
    o << "[piezas]\r\n";
    for (const auto& p : t.piezas) {
        if (fila_vacia(p)) continue;
        o << limpio(p.perfil) << '\t' << limpio(p.nombre) << '\t' << limpio(p.largo) << '\t' << limpio(p.angulo1)
          << '\t' << limpio(p.angulo2) << '\t' << limpio(p.cantidad) << "\r\n";
    }
    return o.str();
}

bool de_texto(const std::string& texto, Trabajo& t, std::string& error) {
    t = Trabajo{};
    std::string s = texto;
    if (s.size() >= 3 && (unsigned char)s[0] == 0xEF && (unsigned char)s[1] == 0xBB && (unsigned char)s[2] == 0xBF)
        s = s.substr(3);   // BOM de UTF-8
    std::istringstream in(s);
    std::string linea;
    int seccion = 0, n = 0;
    bool cabecera = false;
    while (std::getline(in, linea)) {
        n++;
        if (!linea.empty() && linea.back() == '\r') linea.pop_back();
        if (!cabecera) {
            if (linea.rfind("NESTTUBO", 0) != 0) {
                error = "no es un archivo de trabajo de NestTubo";
                return false;
            }
            cabecera = true;
            continue;
        }
        if (linea.empty()) continue;
        if (linea == "[perfiles]") { seccion = 1; continue; }
        if (linea == "[piezas]") { seccion = 2; continue; }
        std::vector<std::string> c;
        size_t a = 0;
        for (;;) {
            size_t b = linea.find('\t', a);
            c.push_back(linea.substr(a, b == std::string::npos ? std::string::npos : b - a));
            if (b == std::string::npos) break;
            a = b + 1;
        }
        if (seccion == 1) {
            PerfilTxt p = perfil_nuevo("");
            std::string* campos[] = {&p.nombre, &p.cara, &p.barra, &p.despunte, &p.zona_muerta, &p.separacion, &p.margen};
            for (size_t i = 0; i < 7 && i < c.size(); i++) *campos[i] = c[i];
            t.perfiles.push_back(p);
        } else if (seccion == 2) {
            PiezaTxt p;
            std::string* campos[] = {&p.perfil, &p.nombre, &p.largo, &p.angulo1, &p.angulo2, &p.cantidad};
            for (size_t i = 0; i < 6 && i < c.size(); i++) *campos[i] = c[i];
            t.piezas.push_back(p);
        } else {
            error = "linea " + std::to_string(n) + ": fuera de las secciones [perfiles] y [piezas]";
            return false;
        }
    }
    if (!cabecera) {
        error = "archivo vacío";
        return false;
    }
    return true;
}

}  // namespace nt
