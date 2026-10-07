// Pruebas del nucleo. Deben terminar en "FALLOS: 0".
//
//   g++ -std=c++17 -O1 -g -fsanitize=address,undefined nucleo.cpp pruebas.cpp -o pruebas
//   ./pruebas ../laboratorio/casos_oraculo.txt
//
// 1. Casos que se calculan a mano (los de laboratorio/pruebas_propio.py).
// 2. Trabajos chicos al azar contra un branch and bound escrito aqui aparte.
// 3. El oraculo: los 1.090 trabajos de laboratorio/casos_oraculo.txt.
// 4. Plan por perfil: posiciones, ids, piezas que no caben, validador.
// 5. Determinismo, tope de tiempo y cancelacion.
// 6. Lectura de numeros con coma o punto.
// 7. El trabajo digitado: lectura, errores, archivo y textos del resultado.
// 7b. Heuristicas rapidas: mismo resultado que las versiones lentas de
//    referencia, y pedidos enormes respetan el tope y la cancelacion.
// 8. Salidas: PDF, Excel y DXF (estructura; la lectura con programas aparte va
//    en compilar.sh salidas).
//
//   ./pruebas --salidas CARPETA trabajo.ntb   escribe el PDF, el Excel y los DXF
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <functional>
#include <thread>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include "nucleo.h"
#include "salidas.h"
#include "trabajo.h"

using namespace nt;

static int fallos = 0;

static void check(bool ok, const std::string& desc) {
    std::printf("%s %s\n", ok ? "OK " : "MAL", desc.c_str());
    if (!ok) fallos++;
}

struct Bin {
    std::vector<i64> w;
    i64 C = 0;
};

static Bin transformar(const std::vector<i64>& largos, i64 L, i64 d, i64 z, i64 s) {
    Bin t;
    t.C = L - d - z + s;
    for (i64 l : largos) t.w.push_back(l + s);
    return t;
}

static std::vector<i64> repetir(i64 l, int n) { return std::vector<i64>(n, l); }

static std::vector<i64> unir(std::initializer_list<std::vector<i64>> partes) {
    std::vector<i64> r;
    for (const auto& p : partes) r.insert(r.end(), p.begin(), p.end());
    return r;
}

// Generador de las pruebas (independiente del que usa el nucleo).
struct Azar {
    uint64_t s;
    explicit Azar(uint64_t x) : s(x * 2654435761ULL + 1) {}
    uint64_t sig() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        return s;
    }
    i64 entre(i64 a, i64 b) { return a + (i64)(sig() % (uint64_t)(b - a + 1)); }
};

// Minimo de barras por branch and bound, solo para trabajos chicos. Escrito aparte
// del nucleo (no usa nada de el) para poder contrastarlo.
static bool exacto_pequeno(std::vector<i64> w, i64 C, int& minimo, long limite_nodos = 2000000) {
    std::sort(w.begin(), w.end(), std::greater<i64>());
    int n = (int)w.size();
    i64 suma = 0;
    for (i64 x : w) suma += x;
    int lb = (int)((suma + C - 1) / C);
    // cota de arranque: first fit
    std::vector<i64> r;
    for (i64 x : w) {
        size_t b = 0;
        while (b < r.size() && r[b] < x) b++;
        if (b == r.size()) r.push_back(C - x);
        else r[b] -= x;
    }
    int mejor = (int)r.size();
    long nodos = 0;
    std::vector<i64> resto;
    std::function<void(int)> rec = [&](int k) {
        if (nodos > limite_nodos || mejor == lb) return;
        nodos++;
        if ((int)resto.size() >= mejor) return;
        if (k == n) {
            mejor = (int)resto.size();
            return;
        }
        std::vector<i64> vistos;
        for (size_t b = 0; b < resto.size(); b++) {
            if (resto[b] < w[k] || std::find(vistos.begin(), vistos.end(), resto[b]) != vistos.end()) continue;
            vistos.push_back(resto[b]);
            resto[b] -= w[k];
            rec(k + 1);
            resto[b] += w[k];
        }
        if ((int)resto.size() + 1 < mejor) {
            resto.push_back(C - w[k]);
            rec(k + 1);
            resto.pop_back();
        }
    };
    rec(0);
    minimo = mejor;
    return nodos <= limite_nodos;
}

static void casos_a_mano() {
    std::puts("== casos a mano");
    struct Caso {
        const char* desc;
        std::vector<i64> largos;
        i64 L, d, z, s;
        int esperado, otro;   // otro = valor alternativo admitido (o -1)
    };
    std::vector<Caso> casos = {
        {"dos mitades exactas, sin separacion", {3000, 3000}, 6000, 0, 0, 0, 1, -1},
        {"dos mitades y 1 mm de mas", {3000, 3000, 1}, 6000, 0, 0, 0, 2, -1},
        {"separacion 3: 3000+2997+3 = 6000 justo", {3000, 2997}, 6000, 0, 0, 3, 1, -1},
        {"separacion 3: 3000+2998+3 = 6001 no cabe", {3000, 2998}, 6000, 0, 0, 3, 2, -1},
        {"zona muerta 230 + despunte 10: util 5760", {2880, 2877}, 6000, 10, 230, 3, 1, -1},
        {"lo mismo con 1 mm mas", {2880, 2878}, 6000, 10, 230, 3, 2, -1},
        {"una pieza sola ocupa todo el util", {5760}, 6000, 10, 230, 3, 1, -1},
        {"seis de 1000 en 6000 sin separacion", repetir(1000, 6), 6000, 0, 0, 0, 1, -1},
        {"seis de 1000 con separacion 3: 6015 > 6000", repetir(1000, 6), 6000, 0, 0, 3, 2, -1},
        {"FFD da 3 aqui; el minimo es 2", {3, 3, 2, 2, 2, 2}, 7, 0, 0, 0, 2, -1},
        // Limite conocido del metodo: el prototipo da 24 y el minimo real es 23
        // (lo demostro exacto_arcflow.py). Se aceptan los dos.
        {"trabajo 289 de la familia medio (limite conocido)",
         unir({repetir(2694, 4), repetir(2443, 17), repetir(1868, 4), repetir(1845, 19), repetir(1611, 19), repetir(445, 7)}),
         6000, 10, 230, 3, 23, 24},
    };
    for (const auto& c : casos) {
        Bin t = transformar(c.largos, c.L, c.d, c.z, c.s);
        ResultadoPesos r = resolver_pesos(t.w, t.C, 5);
        std::string motivo;
        bool ok = validar_pesos(t.w, t.C, r.barras, &motivo);
        int n = (int)r.barras.size();
        bool bien = ok && (n == c.esperado || n == c.otro) && r.cota <= n;
        char buf[200];
        std::snprintf(buf, sizeof buf, "%-52s esperado %d | barras %d cota %lld %s", c.desc, c.esperado, n,
                      (long long)r.cota, n == r.cota ? "(minimo demostrado)" : "(no demostrado)");
        check(bien, buf);
    }
    // FFD y el llenado por separado, en el caso donde FFD falla
    {
        std::vector<i64> w{3, 3, 2, 2, 2, 2};
        check(ffd(w, 7).size() == 3 && bfd(w, 7).size() == 3 && llenado(w, 7).size() == 2,
              "heuristicas sueltas: FFD 3, BFD 3, llenado 2 en {3,3,2,2,2,2} con C 7");
        check(cota_l1(w, 7) == 2 && cota_l2(w, 7) == 2, "cotas L1 y L2 = 2 en ese caso");
    }
    try {
        resolver_pesos({5764}, 5763);
        check(false, "pieza que no cabe: no la rechazo");
    } catch (const std::invalid_argument& e) {
        check(true, std::string("pieza que no cabe -> ") + e.what());
    }
    {
        ResultadoPesos r = resolver_pesos({}, 100);
        check(r.barras.empty() && r.cota == 0, "lista vacia: 0 barras, cota 0");
    }
}

static void contra_branch_and_bound() {
    std::puts("== trabajos chicos contra branch and bound");
    Azar az(11);
    int n = 0, malas = 0, optimas = 0, certificadas = 0;
    i64 capacidades[] = {150, 1000, 5763};
    for (int it = 0; it < 600; it++) {
        i64 C = capacidades[az.entre(0, 2)];
        std::vector<i64> w;
        int piezas = (int)az.entre(4, 13);
        for (int k = 0; k < piezas; k++) w.push_back(az.entre(C / 15, C * 3 / 5));
        int opt;
        if (!exacto_pequeno(w, C, opt)) continue;
        ResultadoPesos r = resolver_pesos(w, C, 5);
        bool ok = validar_pesos(w, C, r.barras);
        n++;
        int b = (int)r.barras.size();
        if (!ok || !(r.cota <= opt && opt <= b) || cota_l2(w, C) > opt) malas++;
        optimas += (b == opt);
        certificadas += (b == r.cota);
    }
    char buf[200];
    std::snprintf(buf, sizeof buf,
                  "%d trabajos chicos: cota <= minimo <= metodo en todos (fallas: %d); metodo = minimo en %d; "
                  "minimo demostrado en %d",
                  n, malas, optimas, certificadas);
    check(malas == 0 && n >= 500, buf);
}

static void oraculo(const std::string& ruta) {
    std::puts("== oraculo");
    std::ifstream f(ruta);
    if (!f) {
        check(false, "no se pudo abrir " + ruta);
        return;
    }
    std::string linea, familia = "?";
    int total = 0, cota_distinta = 0, fuera_rango = 0, invalidas = 0, simples_distintas = 0, mas = 0, menos = 0,
        demostrados = 0, no_convergio = 0, l2_mayor = 0;
    double t_max = 0, t_total = 0;
    std::string peor;
    std::map<std::string, std::pair<double, double>> por_familia;   // suma, max
    std::map<std::string, int> cuantos;
    while (std::getline(f, linea)) {
        if (linea.empty()) continue;
        if (linea[0] == '#') {
            if (linea.rfind("# familia ", 0) == 0) familia = linea.substr(10);
            continue;
        }
        std::istringstream in(linea);
        long long L, d, z, s, cota, metodo, simples;
        std::string sep;
        in >> L >> d >> z >> s >> sep;
        std::vector<i64> largos;
        std::string tok;
        while (in >> tok && tok != ";") {
            size_t p = tok.find(':');
            i64 l = std::stoll(tok.substr(0, p));
            int q = std::stoi(tok.substr(p + 1));
            for (int k = 0; k < q; k++) largos.push_back(l);
        }
        in >> cota >> metodo >> simples;
        Bin t = transformar(largos, L, d, z, s);
        auto t0 = std::chrono::steady_clock::now();
        ResultadoPesos r = resolver_pesos(t.w, t.C, 5);
        double seg = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
        t_total += seg;
        auto& pf = por_familia[familia];
        pf.first += seg;
        pf.second = std::max(pf.second, seg);
        cuantos[familia]++;
        if (seg > t_max) {
            t_max = seg;
            peor = familia + ": " + linea.substr(0, 60);
        }
        total++;
        long long b = (long long)r.barras.size();
        long long mis_simples = std::min({ffd(t.w, t.C).size(), bfd(t.w, t.C).size(), llenado(t.w, t.C).size()});
        if (!validar_pesos(t.w, t.C, r.barras)) invalidas++;
        if (r.cota_lp != cota) {
            cota_distinta++;
            std::printf("    cota distinta (%s): mia %lld, oraculo %lld | %s\n", familia.c_str(), (long long)r.cota_lp,
                        cota, linea.c_str());
        }
        if (r.cota > cota) l2_mayor++;
        if (!(cota <= b && b <= simples)) fuera_rango++;
        if (mis_simples != simples) simples_distintas++;
        if (b > metodo) {
            mas++;
            std::printf("    barras de mas (%s): %lld contra %lld | %s\n", familia.c_str(), b, metodo, linea.c_str());
        }
        if (b < metodo) menos++;
        if (b == r.cota) demostrados++;
        if (!r.info.convergio) no_convergio++;
    }
    char buf[300];
    std::snprintf(buf, sizeof buf, "%d trabajos leidos", total);
    check(total == 1090, buf);
    std::snprintf(buf, sizeof buf, "soluciones validas en todos (invalidas: %d)", invalidas);
    check(invalidas == 0, buf);
    std::snprintf(buf, sizeof buf, "misma cota del LP que el prototipo (distintas: %d)", cota_distinta);
    check(cota_distinta == 0, buf);
    std::snprintf(buf, sizeof buf, "cota <= barras <= simples del prototipo (fuera: %d)", fuera_rango);
    check(fuera_rango == 0, buf);
    std::snprintf(buf, sizeof buf, "heuristicas simples iguales al prototipo (distintas: %d)", simples_distintas);
    check(simples_distintas == 0, buf);
    std::snprintf(buf, sizeof buf, "LP convergio en todos (no: %d)", no_convergio);
    check(no_convergio == 0, buf);
    std::snprintf(buf, sizeof buf,
                  "barras iguales al prototipo salvo casos sueltos (de mas: %d, de menos: %d); minimo demostrado en %d",
                  mas, menos, demostrados);
    check(mas <= 3, buf);
    std::printf("    L2 por encima de la cota del LP en %d trabajos\n", l2_mayor);
    for (const auto& kv : por_familia)
        std::printf("    tiempo %-11s %4d trabajos: media %.4f s, max %.4f s\n", kv.first.c_str(), cuantos[kv.first],
                    kv.second.first / cuantos[kv.first], kv.second.second);
    std::printf("    tiempo total %.2f s; el mas lento %.3f s (%s)\n", t_total, t_max, peor.c_str());
}

static void plan_por_perfil() {
    std::puts("== plan por perfil");
    Parametros p{6000, 10, 230, 3};
    std::vector<Pieza> piezas = {{1, 2000, 3}, {2, 1500, 2}, {3, 6000, 1}, {4, 900, 4}, {5, 2000, 1}};
    Plan plan = calcular_perfil(piezas, p);
    std::string motivo;
    bool valido = validar_plan(piezas, p, plan, &motivo);
    check(valido, "plan valido: " + motivo);
    check(plan.no_caben.size() == 1 && plan.no_caben[0] == 3 && plan.piezas_no_caben == 1,
          "la pieza de 6000 no cabe en util 5760 y queda fuera");
    check(plan.piezas_colocadas == 10, "10 piezas colocadas");
    // 4x2000 + 2x1500 + 4x900 = 14600 mm; con separacion caben en 3 barras (cota L1 = 3)
    check(plan.barras.size() == 3 && plan.demostrado, "3 barras, minimo demostrado");
    bool posiciones = true;
    for (const auto& b : plan.barras) {
        i64 pos = p.despunte;
        for (size_t i = 0; i < b.piezas.size(); i++) {
            if (i) pos += p.separacion;
            posiciones &= b.piezas[i].inicio == pos && b.piezas[i].fin == pos + b.piezas[i].largo;
            pos = b.piezas[i].fin;
        }
    }
    check(posiciones, "inicio y fin de cada pieza: despunte, largo y separacion");
    char buf[200];
    std::snprintf(buf, sizeof buf, "aprovechamiento %.4f = 14600 / 18000", plan.aprovechamiento());
    check(std::fabs(plan.aprovechamiento() - 14600.0 / 18000.0) < 1e-12, buf);

    // Validador: cada alteracion debe rechazarse
    struct Alteracion {
        const char* desc;
        std::function<void(Plan&)> f;
    };
    std::vector<Alteracion> alt = {
        {"quitar una pieza", [](Plan& q) { q.barras[0].piezas.pop_back(); }},
        {"duplicar una pieza", [](Plan& q) { q.barras[0].piezas.push_back(q.barras[0].piezas.back()); }},
        {"alargar una pieza", [](Plan& q) { q.barras[0].piezas[0].largo += 1; q.barras[0].piezas[0].fin += 1; }},
        {"montar dos piezas", [](Plan& q) { q.barras[0].piezas[1].inicio -= 1; q.barras[0].piezas[1].fin -= 1; }},
        {"cambiar el id", [](Plan& q) { q.barras[0].piezas[0].id = 99; }},
        {"vaciar una barra", [](Plan& q) { q.barras.push_back(BarraPlan{}); }},
        {"olvidar la que no cabe", [](Plan& q) { q.no_caben.clear(); }},
        {"contar mal las que no caben", [](Plan& q) { q.piezas_no_caben = 5; }},
        {"meter la que no cabe", [](Plan& q) { q.barras[0].piezas.push_back({3, 6000, 10, 6010}); }},
        {"pasar a la zona muerta",
         [](Plan& q) {
             for (auto& c : q.barras[0].piezas) { c.inicio += 300; c.fin += 300; }
         }},
        {"cota mayor que las barras", [](Plan& q) { q.cota = 99; }},
        {"libre mal calculado", [](Plan& q) { q.barras[0].libre += 1; }},
    };
    for (const auto& a : alt) {
        Plan q = plan;
        a.f(q);
        bool rechaza = !validar_plan(piezas, p, q, &motivo);
        check(rechaza, std::string("validador rechaza: ") + a.desc + " -> " + motivo);
    }

    // barras iguales agrupadas
    std::vector<Pieza> iguales = {{1, 2800, 10}};
    Plan pi = calcular_perfil(iguales, p);
    auto g = agrupar(pi);
    check(pi.barras.size() == 5 && g.size() == 1 && g[0].veces == 5, "10 x 2800: 5 barras iguales -> un grupo x 5");

    // mismo largo con dos ids: se reparten en orden y no se mezclan los grupos
    std::vector<Pieza> dos = {{7, 2800, 2}, {8, 2800, 2}};
    Plan pd = calcular_perfil(dos, p);
    auto gd = agrupar(pd);
    check(validar_plan(dos, p, pd) && pd.barras.size() == 2 && gd.size() == 2,
          "dos ids con el mismo largo: 2 barras, 2 grupos");

    // parametros sin sentido
    bool lanza = false;
    try {
        calcular_perfil(piezas, Parametros{200, 10, 230, 3});
    } catch (const std::invalid_argument&) {
        lanza = true;
    }
    check(lanza, "despunte + zona muerta mayor que la barra: error claro");

    // decimas de mm: 1234,5 y 2345,5 con separacion 3,0 en una barra de 6000,0
    Parametros pdm{60000, 100, 2300, 30};
    std::vector<Pieza> dec = {{1, 12345, 2}, {2, 23455, 1}};
    Plan pdec = calcular_perfil(dec, pdm);
    check(validar_plan(dec, pdm, pdec) && pdec.barras.size() == 1, "decimas: 2x1234,5 + 2345,5 en una barra");
}

static void determinismo_y_tiempo(const std::string& ruta) {
    std::puts("== determinismo, tope de tiempo y cancelacion");
    // un trabajo grande del oraculo (familia variado: muchos largos distintos)
    std::ifstream f(ruta);
    std::string linea, ultima;
    while (std::getline(f, linea))
        if (!linea.empty() && linea[0] != '#') ultima = linea;
    std::vector<Pieza> piezas;
    Parametros p;
    {
        std::istringstream in(ultima);
        std::string sep, tok;
        in >> p.largo_barra >> p.despunte >> p.zona_muerta >> p.separacion >> sep;
        int id = 1;
        while (in >> tok && tok != ";") {
            size_t k = tok.find(':');
            piezas.push_back({id++, std::stoll(tok.substr(0, k)), std::stoi(tok.substr(k + 1))});
        }
    }
    Plan a = calcular_perfil(piezas, p);
    Plan b = calcular_perfil(piezas, p);
    bool iguales = a.barras.size() == b.barras.size();
    for (size_t k = 0; iguales && k < a.barras.size(); k++) {
        iguales = a.barras[k].piezas.size() == b.barras[k].piezas.size();
        for (size_t i = 0; iguales && i < a.barras[k].piezas.size(); i++)
            iguales = a.barras[k].piezas[i].id == b.barras[k].piezas[i].id &&
                      a.barras[k].piezas[i].inicio == b.barras[k].piezas[i].inicio;
    }
    char buf[200];
    std::snprintf(buf, sizeof buf, "misma lista, mismo plan (%zu piezas distintas, %zu barras)", piezas.size(),
                  a.barras.size());
    check(iguales && validar_plan(piezas, p, a), buf);

    Control vencido;
    vencido.limite = std::chrono::steady_clock::now();
    Plan v = calcular_perfil(piezas, p, 5, &vencido);
    std::snprintf(buf, sizeof buf, "tope de tiempo ya vencido: plan valido igual (%zu barras, cota %lld, %s)",
                  v.barras.size(), (long long)v.cota, v.estado == Estado::tiempo_agotado ? "tiempo agotado" : "?");
    check(validar_plan(piezas, p, v) && v.estado == Estado::tiempo_agotado && v.cota <= (i64)v.barras.size(), buf);

    std::atomic<bool> cancelar{true};
    Control cc;
    cc.cancelar = &cancelar;
    Plan c = calcular_perfil(piezas, p, 5, &cc);
    check(validar_plan(piezas, p, c) && c.estado == Estado::cancelado, "cancelado antes de empezar: plan valido igual");
}

static void lectura_numeros() {
    std::puts("== lectura de numeros");
    struct C {
        const char* t;
        bool ok;
        long long v;
    };
    C casos[] = {{"1234", true, 12340}, {"1234,5", true, 12345}, {"1234.5", true, 12345}, {" 90 ", true, 900},
                 {",5", true, 5},       {"7,", true, 70},        {"1,25", false, 0},     {"1.234,5", false, 0},
                 {"-3", false, 0},      {"", false, 0},          {"abc", false, 0},      {"12a", false, 0},
                 {",", false, 0}};
    bool todo = true;
    for (const auto& c : casos) {
        i64 v = -1;
        bool ok = leer_decimas(c.t, v);
        if (ok != c.ok || (ok && v != c.v)) {
            std::printf("    \"%s\": %s %lld\n", c.t, ok ? "acepta" : "rechaza", (long long)v);
            todo = false;
        }
    }
    check(todo, "coma o punto decimal, un decimal como mucho, sin signos ni letras");
}


static void trabajo_digitado() {
    std::puts("== trabajo digitado");
    Trabajo t;
    t.perfiles = {perfil_nuevo("Cuadrado 40x40x2"), perfil_nuevo("Rect 50x25")};
    t.perfiles[1].barra = "6000,0";
    t.piezas = {{"cuadrado 40x40x2", "Larguero", "1250", "", "45", "8"},
                {"Rect 50x25", "Travesano", "640,5", "0", "0", "12"},
                {"", "", "", "", "", ""},
                {"Cuadrado 40x40x2", "", "2000", "", "", "3"}};
    std::vector<ProblemaPerfil> pr;
    std::vector<std::string> err;
    bool ok = preparar(t, pr, err);
    check(ok && pr.size() == 2, "dos perfiles con piezas -> dos problemas (fila vacia ignorada, perfil sin distinguir mayusculas)");
    if (ok && pr.size() == 2) {
        check(pr[0].escala == 1 && pr[0].param.largo_barra == 6000 && pr[0].param.zona_muerta == 230 &&
                  pr[0].piezas.size() == 2 && pr[0].piezas[1].id == 3 && pr[0].nombres[3] == "Fila 4",
              "perfil entero en mm; pieza sin nombre se llama por su fila");
        check(pr[1].escala == 10 && pr[1].param.largo_barra == 60000 && pr[1].param.separacion == 30 &&
                  pr[1].piezas[0].largo == 6405,
              "perfil con un decimal -> decimas de mm");
    }
    auto con_error = [&](Trabajo tt, const std::string& busca, const std::string& desc) {
        std::vector<ProblemaPerfil> p2;
        std::vector<std::string> e2;
        bool ok2 = preparar(tt, p2, e2);
        bool hay = false;
        for (const auto& e : e2) hay |= e.find(busca) != std::string::npos;
        check(!ok2 && hay, desc + " -> " + (e2.empty() ? std::string("(sin error)") : e2[0]));
    };
    Trabajo m = t;
    m.piezas[0].perfil = "Redondo 2";
    con_error(m, "no está en la tabla", "perfil que no existe");
    m = t;
    m.piezas[0].largo = "12a";
    con_error(m, "no es un número", "largo con letras");
    m = t;
    m.piezas[0].angulo1 = "90";
    con_error(m, "menor que 90", "angulo de 90 (0 = recto)");
    m = t;
    m.piezas[0].cantidad = "0";
    con_error(m, "entero entre 1", "cantidad 0");
    m = t;
    m.piezas[0].cantidad = "2,5";
    con_error(m, "entero entre 1", "cantidad con decimal");
    m = t;
    m.perfiles.push_back(perfil_nuevo("RECT 50X25"));
    con_error(m, "repetido", "perfil repetido");
    m = t;
    m.perfiles[0].zona_muerta = "5990";
    con_error(m, "se comen toda la barra", "zona muerta mayor que la barra");
    m = t;
    m.piezas.clear();
    con_error(m, "No hay piezas", "sin piezas");

    // archivo: ida y vuelta, y archivo viejo con campos de menos
    Trabajo r;
    std::string error;
    check(de_texto(a_texto(t), r, error) && a_texto(r) == a_texto(t) && r.piezas.size() == 3,
          "archivo: guardar y abrir deja lo mismo (sin filas vacias)");
    check(de_texto("NESTTUBO\t1\r\n[perfiles]\r\nViejo\t\t6000\r\n[piezas]\r\nViejo\tA\t100\r\n", r, error) &&
              r.perfiles[0].zona_muerta == "230" && r.perfiles[0].margen == "0" && r.piezas[0].cantidad.empty(),
          "archivo viejo con campos de menos: se rellenan los del perfil");
    bool ajeno = !de_texto("hola", r, error);
    check(ajeno, "archivo ajeno: rechazado (" + error + ")");

    // textos del resultado
    Parametros p{6000, 10, 230, 3};
    ResultadoPerfil res;
    res.prob.nombre = "Cuadrado";
    res.prob.param = p;
    res.prob.margen = 1;
    res.prob.piezas = {{0, 2800, 10}, {1, 7000, 1}};
    res.prob.nombres = {"Poste", "Viga"};
    res.plan = calcular_perfil(res.prob.piezas, p);
    res.valido = validar_plan(res.prob.piezas, p, res.plan, &res.motivo);
    auto fr = fila_resumen(res);
    check(fr[2] == "6" && fr[3] == "5" && fr[5] == "10 (+1 no caben)" && fr[7] == "sí",
          "resumen: 5 barras + margen 1 = 6 a enviar; minimo demostrado");
    auto fp = filas_plan(res);
    std::vector<std::string> f0{"1-5 (× 5)", "Poste", "2800", "10", "2810", "387"}, f1{"", "Poste", "2800", "2813", "5613", ""};
    check(fp.size() == 3 && fp[0] == f0 && fp[1] == f1 && fp[2][0] == "NO CABE" && fp[2][1] == "Viga",
          "plan: barras iguales agrupadas, inicio y fin, sobrante 6000 - 5613 = 387, la que no cabe al final");
    check(medida(12345, 10) == "1234,5" && medida(12340, 10) == "1234" && medida(77, 1) == "77", "medidas con coma decimal");
    // las que no caben se cuentan por unidades, no por filas
    res.prob.piezas = {{0, 2800, 10}, {1, 7000, 8}};
    res.plan = calcular_perfil(res.prob.piezas, p);
    res.valido = validar_plan(res.prob.piezas, p, res.plan, &res.motivo);
    fr = fila_resumen(res);
    fp = filas_plan(res);
    check(res.valido && fr[5] == "10 (+8 no caben)" && fp.back()[1] == "Viga (× 8)",
          "8 vigas que no caben: \"+8 no caben\" y \"Viga (× 8)\" en el plan (" + fr[5] + ")");
}

static const char* TRABAJO_EJEMPLO =
    "NESTTUBO\t1\n[perfiles]\n"
    "Cuadrado 40x40x2\t40\t6000\t10\t230\t3\t0\n"
    "Ángulo 30x30x3\t30\t6400\t10\t230\t3\t1\n"
    "Rect 80/40\t\t6000\t10\t230\t3\t0\n"
    "[piezas]\n"
    "Cuadrado 40x40x2\tLarguero\t1250\t0\t45\t8\n"
    "Cuadrado 40x40x2\tTravesaño\t640,5\t0\t0\t12\n"
    "Ángulo 30x30x3\tRefuerzo\t455\t45\t45\t30\n"
    "Ángulo 30x30x3\tViga demasiado larga\t6300\t0\t0\t1\n"
    "Rect 80/40\tPoste (A)\t2400\t0\t0\t6\n";

static std::vector<ResultadoPerfil> resolver_texto(const std::string& texto, std::string& error) {
    Trabajo t;
    std::vector<ResultadoPerfil> rs;
    if (!de_texto(texto, t, error)) return rs;
    std::vector<ProblemaPerfil> pr;
    std::vector<std::string> err;
    if (!preparar(t, pr, err)) {
        error = err.empty() ? "?" : err[0];
        return rs;
    }
    for (const auto& p : pr) rs.push_back(resolver(p));
    return rs;
}

static uint32_t le32(const std::string& s, size_t i) {
    return (uint32_t)(unsigned char)s[i] | (uint32_t)(unsigned char)s[i + 1] << 8 | (uint32_t)(unsigned char)s[i + 2] << 16 |
           (uint32_t)(unsigned char)s[i + 3] << 24;
}

static int contar(const std::string& s, const std::string& que) {
    int n = 0;
    for (size_t i = s.find(que); i != std::string::npos; i = s.find(que, i + 1)) n++;
    return n;
}

// Versiones directas de FFD, BFD y llenado, como en modelo.py: la referencia
// contra la que se comparan las de nucleo.cpp (arbol, conjunto y copias saltadas).
static std::vector<int> orden_ref(const std::vector<i64>& w) {
    std::vector<int> o(w.size());
    for (size_t i = 0; i < o.size(); i++) o[i] = (int)i;
    std::stable_sort(o.begin(), o.end(), [&](int a, int b) { return w[a] > w[b]; });
    return o;
}
static std::vector<std::vector<int>> ffd_ref(const std::vector<i64>& w, i64 C) {
    std::vector<std::vector<int>> barras;
    std::vector<i64> resto;
    for (int i : orden_ref(w)) {
        size_t b = 0;
        while (b < barras.size() && resto[b] < w[i]) b++;
        if (b == barras.size()) { barras.push_back({i}); resto.push_back(C - w[i]); }
        else { barras[b].push_back(i); resto[b] -= w[i]; }
    }
    return barras;
}
static std::vector<std::vector<int>> bfd_ref(const std::vector<i64>& w, i64 C) {
    std::vector<std::vector<int>> barras;
    std::vector<i64> resto;
    for (int i : orden_ref(w)) {
        int mb = -1;
        for (size_t b = 0; b < barras.size(); b++)
            if (resto[b] >= w[i] && (mb < 0 || resto[b] < resto[mb])) mb = (int)b;
        if (mb < 0) { barras.push_back({i}); resto.push_back(C - w[i]); }
        else { barras[mb].push_back(i); resto[mb] -= w[i]; }
    }
    return barras;
}
static std::vector<std::vector<int>> llenado_ref(const std::vector<i64>& w, i64 C) {
    std::vector<int> pend = orden_ref(w);
    std::vector<std::vector<int>> barras;
    while (!pend.empty()) {
        i64 resto = C - w[pend[0]];
        std::vector<char> alc((size_t)resto + 1, 0);
        std::vector<int> quien((size_t)resto + 1, -1);
        alc[0] = 1;
        for (size_t k = 1; k < pend.size(); k++) {
            i64 p = w[pend[k]];
            for (i64 c = resto; c >= p; c--)   // de arriba abajo: cada pieza una vez
                if (!alc[c] && alc[c - p]) { alc[c] = 1; quien[c] = (int)k; }
        }
        i64 c = resto;
        while (c > 0 && !alc[c]) c--;
        std::vector<int> elegido{pend[0]};
        std::vector<char> usado(pend.size(), 0);
        usado[0] = 1;
        while (c > 0) {
            int k = quien[c];
            elegido.push_back(pend[k]);
            usado[k] = 1;
            c -= w[pend[k]];
        }
        barras.push_back(elegido);
        std::vector<int> quedan;
        for (size_t k = 0; k < pend.size(); k++)
            if (!usado[k]) quedan.push_back(pend[k]);
        pend.swap(quedan);
    }
    return barras;
}

static double segundos_desde(std::chrono::steady_clock::time_point t0) {
    return std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
}

static void heuristicas_rapidas() {
    std::puts("== heuristicas rapidas");
    // 1. Mismo resultado, barra por barra, que las versiones directas
    uint64_t s = 12345;
    auto azar = [&](uint64_t n) {
        s = s * 6364136223846793005ULL + 1442695040888963407ULL;
        return (s >> 33) % n;
    };
    int distintos = 0, casos = 0;
    for (int t = 0; t < 800; t++) {
        i64 C = t % 4 == 0 ? 57630 : (i64)(50 + azar(6000));
        int tipos = 1 + (int)azar(t % 3 == 0 ? 4 : 25);
        std::vector<i64> largos;
        for (int k = 0; k < tipos; k++) largos.push_back(1 + (i64)azar((uint64_t)C));
        if (t % 5 == 0) largos.push_back(C);            // pieza que llena la barra
        if (t % 7 == 0) largos.push_back(C / 2);        // mitades exactas
        std::vector<i64> w;
        int n = 1 + (int)azar(t % 4 == 0 ? 60 : 160);
        for (int i = 0; i < n; i++) w.push_back(largos[azar(largos.size())]);
        casos++;
        if (ffd(w, C) != ffd_ref(w, C) || bfd(w, C) != bfd_ref(w, C) || llenado(w, C) != llenado_ref(w, C)) distintos++;
    }
    check(distintos == 0, std::to_string(casos) + " listas al azar: FFD, BFD y llenado dan lo mismo que las versiones directas (" +
                              std::to_string(distintos) + " distintas)");

    // 2. Pedidos enormes: el tope y la cancelacion se respetan y el plan es valido
    Parametros pd{60000, 100, 2300, 30};   // en decimas de mm
    std::vector<Pieza> grande;
    i64 largos[] = {25000, 12000, 9000, 18000, 7000};
    for (int k = 0; k < 5; k++) grande.push_back({k, largos[k], 4000});   // 20.000 piezas
    Control tope;
    tope.limite = std::chrono::steady_clock::now() + std::chrono::milliseconds(1000);
    auto t0 = std::chrono::steady_clock::now();
    Plan g = calcular_perfil(grande, pd, 5, &tope);
    double t = segundos_desde(t0);
    char buf[240];
    std::snprintf(buf, sizeof buf, "20.000 piezas en decimas, tope 1 s: %.2f s, %zu barras, cota %lld", t, g.barras.size(),
                  (long long)g.cota);
    check(validar_plan(grande, pd, g) && t < 2.5, buf);

#ifdef __SANITIZE_ADDRESS__
    const int cuantas = 50000;   // con sanitizadores todo va unas 8 veces mas lento
#else
    const int cuantas = 100000;  // la cantidad maxima de una fila en la ventana
#endif
    std::vector<Pieza> fila{{0, 500, cuantas}};
    Parametros pm{6000, 10, 230, 3};
    Control tope20;
    tope20.limite = std::chrono::steady_clock::now() + std::chrono::seconds(20);
    t0 = std::chrono::steady_clock::now();
    Plan f = calcular_perfil(fila, pm, 5, &tope20);
    t = segundos_desde(t0);
    std::snprintf(buf, sizeof buf, "%d piezas de 500 mm, tope 20 s: %.2f s, %zu barras, cota %lld", cuantas, t, f.barras.size(),
                  (long long)f.cota);
    check(validar_plan(fila, pm, f) && f.estado == Estado::completo && f.demostrado && t < 20, buf);

    std::atomic<bool> cancelar{false};
    Control cc;
    cc.cancelar = &cancelar;
    std::vector<Pieza> otra;
    for (int k = 0; k < 5; k++) otra.push_back({k, largos[k] + 1, 8000});   // 40.000 piezas
    Plan c;
    std::thread hilo([&] { c = calcular_perfil(otra, pd, 5, &cc); });
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    t0 = std::chrono::steady_clock::now();
    cancelar = true;
    hilo.join();
    t = segundos_desde(t0);
    std::snprintf(buf, sizeof buf, "40.000 piezas, cancelar a los 0,1 s: responde en %.2f s con un plan valido", t);
    check(validar_plan(otra, pd, c) && t < 1.5, buf);
}

static void salidas() {
    std::puts("== salidas");
    check(a_winansi("Ángulo × · € →") == "\xC1ngulo \xD7 \xB7 \x80 ?", "WinAnsi: tildes, por, punto medio, euro; lo demas '?'");
    check(sin_tildes("Ángulo Travesaño ñ ×") == "Angulo Travesano n x", "sin tildes para el DXF");
    std::string z = zip_sin_comprimir({{"a.txt", "123456789"}});
    check(le32(z, 0) == 0x04034b50 && le32(z, 14) == 0xCBF43926 && le32(z, z.size() - 22) == 0x06054b50,
          "ZIP: cabecera, CRC32 de \"123456789\" = CBF43926, directorio al final");

    std::string error;
    auto rs = resolver_texto(TRABAJO_EJEMPLO, error);
    bool ok = rs.size() == 3 && rs[0].valido && rs[1].valido && rs[2].valido;
    check(ok, "trabajo de ejemplo calculado " + error);
    if (!ok) return;

    // PDF: la tabla xref apunta a cada objeto y el texto va en WinAnsi
    std::string pdf = informe_pdf(rs, "Ejemplo", "07/10/2026");
    size_t sx = pdf.rfind("startxref\n");
    bool xref = pdf.compare(0, 9, "%PDF-1.4\n") == 0 && sx != std::string::npos;
    size_t px = xref ? std::stoul(pdf.substr(sx + 10)) : 0;
    xref = xref && pdf.compare(px, 5, "xref\n") == 0;
    int objetos = 0;
    if (xref) {
        std::istringstream in(pdf.substr(px + 5));
        int desde, n;
        in >> desde >> n;
        std::string linea;
        std::getline(in, linea);
        std::getline(in, linea);   // el objeto 0
        for (int k = 1; k < n && xref; k++) {
            std::getline(in, linea);
            size_t off = std::stoul(linea.substr(0, 10));
            std::string cab = std::to_string(k) + " 0 obj\n";
            xref = linea.size() == 19 && pdf.compare(off, cab.size(), cab) == 0;   // 20 con el \n
            objetos++;
        }
    }
    check(xref && objetos >= 6, "PDF: xref con " + std::to_string(objetos) + " objetos, cada uno donde dice");
    check(pdf.find("(Barras a enviar al proveedor de corte l\\341ser) Tj") != std::string::npos &&
              pdf.find("(Travesa\\361o") != std::string::npos,
          "PDF: titulo y nombres con tilde en WinAnsi");
    check(pdf.find("/Count 4") != std::string::npos, "PDF: resumen + una pagina por perfil = 4 paginas");
    check(informe_pdf(rs, "Ejemplo", "07/10/2026") == pdf, "PDF: mismo trabajo, mismos bytes");

    // Excel: 8 partes, texto con tildes en UTF-8, total de barras
    std::string xl = informe_xlsx(rs, "Ejemplo", "07/10/2026");
    i64 total = 0;
    for (const auto& r : rs) total += r.barras_enviar();
    check(contar(xl, std::string("PK\x03\x04", 4)) == 8 && xl.find("Travesaño") != std::string::npos &&
              xl.find("<sheet name=\"Plan\"") != std::string::npos &&
              xl.find("<c r=\"C" + std::to_string(rs.size() + 3) + "\" s=\"3\"><v>" + std::to_string(total) + "</v>") !=
                  std::string::npos,
          "Excel: 8 partes, tildes en UTF-8, total " + std::to_string(total) + " barras al pie del resumen");

    // DXF: una por distribucion, ASCII, lineas contadas, nombres validos en Windows
    size_t grupos = 0, archivos = 0;
    bool ascii = true, nombres = true, lineas = true;
    for (const auto& r : rs) {
        auto g = agrupar(r.plan);
        auto ds = dxf_perfil(r);
        grupos += g.size();
        archivos += ds.size();
        for (size_t i = 0; i < ds.size() && i < g.size(); i++) {
            for (unsigned char c : ds[i].texto) ascii &= c < 128;
            for (char c : ds[i].nombre) nombres &= std::string("<>:\"/\\|?*").find(c) == std::string::npos && (unsigned char)c < 128;
            size_t piezas = r.plan.barras[g[i].primera].piezas.size();
            lineas &= contar(ds[i].texto, "0\r\nLINE\r\n") == (int)(4 + 1 + 3 + 4 * piezas);
            lineas &= ds[i].texto.size() > 8 && ds[i].texto.compare(ds[i].texto.size() - 8, 8, "0\r\nEOF\r\n") == 0;
        }
    }
    check(grupos == archivos && ascii && nombres && lineas,
          "DXF: " + std::to_string(archivos) + " archivos, uno por distribucion, ASCII, 4 lineas por pieza, nombres validos");
    auto d1 = dxf_perfil(rs[1]);
    check(!d1.empty() && d1[0].nombre.rfind("Angulo 30x30x3 - barra", 0) == 0, "DXF: nombre sin tilde (" +
                                                                                  (d1.empty() ? "" : d1[0].nombre) + ")");
    auto d2 = dxf_perfil(rs[2]);
    check(!d2.empty() && d2[0].nombre.rfind("Rect 80-40 - barra", 0) == 0 &&
              d2[0].texto.find("cara no indicada") != std::string::npos,
          "DXF: '/' fuera del nombre; perfil sin cara se dibuja de 50 y lo dice");

    // trapecio: pieza de 1000 con 0 y 45 grados en cara 40 -> arriba retrocede 40 en el extremo 2
    ResultadoPerfil t;
    t.prob.nombre = "T";
    t.prob.param = Parametros{6000, 10, 230, 3};
    t.prob.cara = 40;
    t.prob.piezas = {{0, 1000, 1}};
    t.prob.nombres = {"P"};
    t.prob.angulo1 = {0};
    t.prob.angulo2 = {450};
    t = resolver(t.prob);
    auto dt = dxf_perfil(t);
    auto seg = [](double x1, double y1, double x2, double y2) {
        char b[200];
        std::snprintf(b, sizeof b, "10\r\n%.3f\r\n20\r\n%.3f\r\n30\r\n0.0\r\n11\r\n%.3f\r\n21\r\n%.3f\r\n", x1, y1, x2, y2);
        return std::string(b);
    };
    check(dt.size() == 1 && dt[0].texto.find(seg(1010, 0, 970, 40)) != std::string::npos &&
              dt[0].texto.find(seg(10, 40, 10, 0)) != std::string::npos,
          "DXF: extremo a 45 grados en cara 40 retrocede 40 arriba; extremo a 0 vertical");
}

// Escribe el PDF, el Excel y los DXF de un trabajo, para revisarlos con otros programas.
static int escribir_salidas(const std::string& carpeta, const std::string& ruta) {
    std::ifstream in(ruta, std::ios::binary);
    std::stringstream ss;
    ss << in.rdbuf();
    std::string error;
    auto rs = resolver_texto(ss.str(), error);
    if (rs.empty()) {
        std::fprintf(stderr, "%s: %s\n", ruta.c_str(), error.c_str());
        return 1;
    }
    namespace fs = std::filesystem;
    std::string nombre = fs::path(ruta).stem().string();
    fs::create_directories(fs::path(carpeta) / (nombre + " - DXF"));
    auto escribir = [](const fs::path& p, const std::string& datos) {
        std::ofstream o(p, std::ios::binary);
        o << datos;
        std::printf("%s (%zu bytes)\n", p.string().c_str(), datos.size());
    };
    escribir(fs::path(carpeta) / (nombre + " - plan.pdf"), informe_pdf(rs, nombre, "07/10/2026"));
    escribir(fs::path(carpeta) / (nombre + " - plan.xlsx"), informe_xlsx(rs, nombre, "07/10/2026"));
    for (const auto& r : rs)
        for (const auto& d : dxf_perfil(r)) escribir(fs::path(carpeta) / (nombre + " - DXF") / d.nombre, d.texto);
    return 0;
}

int main(int argc, char** argv) {
    if (argc == 4 && std::string(argv[1]) == "--salidas") return escribir_salidas(argv[2], argv[3]);
    std::string ruta = argc > 1 ? argv[1] : "../laboratorio/casos_oraculo.txt";
    casos_a_mano();
    contra_branch_and_bound();
    plan_por_perfil();
    lectura_numeros();
    trabajo_digitado();
    heuristicas_rapidas();
    salidas();
    determinismo_y_tiempo(ruta);
    oraculo(ruta);
    std::printf("\nFALLOS: %d\n", fallos);
    return fallos ? 1 : 0;
}
