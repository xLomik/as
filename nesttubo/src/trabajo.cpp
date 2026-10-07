// NestTubo - lectura del trabajo digitado y textos del resultado (sin Win32).
#include "trabajo.h"

#include <algorithm>
#include <cstdio>
#include <map>
#include <sstream>
#include <stdexcept>

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
    std::vector<i64> ang1(t.piezas.size(), 0), ang2(t.piezas.size(), 0);
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
        ang1[i] = a1;
        ang2[i] = a2;
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
        pr.angulo1 = ang1;
        pr.angulo2 = ang2;
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

const Pieza* pieza_de(const ProblemaPerfil& p, int id) {
    for (const Pieza& pz : p.piezas)
        if (pz.id == id) return &pz;
    return nullptr;
}

std::string nombre_no_cabe(const ProblemaPerfil& p, int id) {
    const Pieza* pz = pieza_de(p, id);
    std::string n = id >= 0 && id < (int)p.nombres.size() ? p.nombres[id] : "?";
    if (pz && pz->cantidad > 1) n += " (× " + std::to_string(pz->cantidad) + ")";
    return n;
}

ResultadoPerfil resolver(const ProblemaPerfil& prob, const Control* control) {
    ResultadoPerfil r;
    r.prob = prob;
    try {
        r.plan = calcular_perfil(r.prob.piezas, r.prob.param, 5, control);
        // toda solucion pasa por el validador antes de mostrarse
        r.valido = validar_plan(r.prob.piezas, r.prob.param, r.plan, &r.motivo);
        if (!r.valido) r.motivo = "plan rechazado por el validador: " + r.motivo;
    } catch (const std::exception& e) {
        r.valido = false;
        r.motivo = e.what();
    }
    return r;
}

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
    if (p.piezas_no_caben) piezas += " (+" + miles(p.piezas_no_caben) + " no caben)";
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
        const Pieza* pz = pieza_de(r.prob, id);
        filas.push_back({"NO CABE", nombre_no_cabe(r.prob, id), medida(pz ? pz->largo : 0, e), "", "",
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

std::string clave_perfil(const std::string& nombre) { return minusculas(nombre); }

namespace {
int buscar(const std::vector<PerfilTxt>& v, const std::string& clave) {
    for (size_t i = 0; i < v.size(); i++)
        if (clave_perfil(v[i].nombre) == clave) return (int)i;
    return -1;
}
bool esta(const std::vector<std::string>& v, const std::string& x) { return std::find(v.begin(), v.end(), x) != v.end(); }
}  // namespace

void juntar_cambios(CambiosCatalogo& viejos, const CambiosCatalogo& nuevos) {
    for (const std::string& q : nuevos.quitados) {
        std::string k = clave_perfil(q);
        int i = buscar(viejos.cambiados, k);
        if (i >= 0) viejos.cambiados.erase(viejos.cambiados.begin() + i);
        if (!esta(viejos.quitados, k)) viejos.quitados.push_back(k);
    }
    for (const PerfilTxt& p : nuevos.cambiados) {
        std::string k = clave_perfil(p.nombre);
        if (k.empty()) continue;
        auto it = std::find(viejos.quitados.begin(), viejos.quitados.end(), k);
        if (it != viejos.quitados.end()) viejos.quitados.erase(it);
        int i = buscar(viejos.cambiados, k);
        if (i >= 0) viejos.cambiados[i] = p;
        else viejos.cambiados.push_back(p);
    }
}

std::vector<PerfilTxt> aplicar_cambios(const std::vector<PerfilTxt>& catalogo, const CambiosCatalogo& c) {
    std::vector<PerfilTxt> r;
    for (const PerfilTxt& p : catalogo) {
        std::string k = clave_perfil(p.nombre);
        if (k.empty() || buscar(r, k) >= 0) continue;
        bool quitado = false;
        for (const std::string& q : c.quitados) quitado |= clave_perfil(q) == k;
        if (!quitado) r.push_back(p);
    }
    for (const PerfilTxt& p : c.cambiados) {
        std::string k = clave_perfil(p.nombre);
        if (k.empty()) continue;
        int i = buscar(r, k);
        if (i >= 0) r[i] = p;
        else r.push_back(p);
    }
    return r;
}

CambiosCatalogo cambios_de_tabla(const std::vector<PerfilTxt>& tabla, const std::vector<std::string>& tocados,
                                 const std::vector<std::string>& quitados) {
    CambiosCatalogo c;
    for (const std::string& t : tocados) {
        std::string k = clave_perfil(t);
        int i = buscar(tabla, k);
        if (!k.empty() && i >= 0 && buscar(c.cambiados, k) < 0) c.cambiados.push_back(tabla[i]);
    }
    for (const std::string& q : quitados) {
        std::string k = clave_perfil(q);
        if (!k.empty() && buscar(tabla, k) < 0 && !esta(c.quitados, k)) c.quitados.push_back(k);
    }
    return c;
}

std::vector<PerfilTxt> unir_catalogos(const std::vector<PerfilTxt>& carpeta, const std::vector<PerfilTxt>& local) {
    CambiosCatalogo solo_local;
    for (const PerfilTxt& p : local)
        if (!clave_perfil(p.nombre).empty() && buscar(carpeta, clave_perfil(p.nombre)) < 0) solo_local.cambiados.push_back(p);
    return aplicar_cambios(carpeta, solo_local);
}

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

// Cambios pendientes del catalogo:
//
//   NESTTUBO-CAMBIOS<TAB>1
//   [cambiados]
//   nombre<TAB>cara<TAB>barra<TAB>despunte<TAB>zona_muerta<TAB>separacion<TAB>margen
//   [quitados]
//   nombre

std::string a_texto_cambios(const CambiosCatalogo& c) {
    std::ostringstream o;
    o << "NESTTUBO-CAMBIOS\t1\r\n[cambiados]\r\n";
    for (const auto& p : c.cambiados)
        o << limpio(p.nombre) << '\t' << limpio(p.cara) << '\t' << limpio(p.barra) << '\t' << limpio(p.despunte) << '\t'
          << limpio(p.zona_muerta) << '\t' << limpio(p.separacion) << '\t' << limpio(p.margen) << "\r\n";
    o << "[quitados]\r\n";
    for (const auto& q : c.quitados) o << limpio(q) << "\r\n";
    return o.str();
}

bool de_texto_cambios(const std::string& texto, CambiosCatalogo& c, std::string& error) {
    c = CambiosCatalogo{};
    std::istringstream in(texto);
    std::string linea;
    int seccion = 0;
    bool cabecera = false;
    while (std::getline(in, linea)) {
        if (!linea.empty() && linea.back() == '\r') linea.pop_back();
        if (!cabecera) {
            if (linea.rfind("NESTTUBO-CAMBIOS", 0) != 0) {
                error = "no es un archivo de cambios de NestTubo";
                return false;
            }
            cabecera = true;
            continue;
        }
        if (linea.empty()) continue;
        if (linea == "[cambiados]") { seccion = 1; continue; }
        if (linea == "[quitados]") { seccion = 2; continue; }
        if (seccion == 1) {
            PerfilTxt p = perfil_nuevo("");
            std::string* campos[] = {&p.nombre, &p.cara, &p.barra, &p.despunte, &p.zona_muerta, &p.separacion, &p.margen};
            size_t a = 0;
            for (int i = 0; i < 7; i++) {
                size_t b = linea.find('\t', a);
                *campos[i] = linea.substr(a, b == std::string::npos ? std::string::npos : b - a);
                if (b == std::string::npos) break;
                a = b + 1;
            }
            c.cambiados.push_back(p);
        } else if (seccion == 2) {
            c.quitados.push_back(linea);
        } else {
            error = "linea fuera de [cambiados] y [quitados]";
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
