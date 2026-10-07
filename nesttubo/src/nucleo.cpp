// NestTubo - nucleo de calculo. Port de laboratorio/simplex_propio.py y modelo.py.
// Sin Win32: se compila nativo para las pruebas y se enlaza tal cual en el .exe.
#include "nucleo.h"

#include <algorithm>
#include <cmath>
#include <deque>
#include <limits>
#include <numeric>
#include <set>
#include <stdexcept>

namespace nt {

namespace {

constexpr double EPS_COSTO = 1e-7;      // un patron entra solo si vale mas que 1 + EPS_COSTO
constexpr double EPS_PRECIO = 1e-9;     // un precio por debajo de -EPS_PRECIO hace entrar la holgura de ese largo
constexpr double EPS_PERTURBA = 1e-6;   // tamano de la perturbacion de la demanda que evita el ciclado
constexpr double EPS_PIVOTE = 1e-9;     // componentes menores no sirven de pivote

// Generador propio con semilla fija: la misma lista da siempre el mismo plan.
struct Aleatorio {
    uint64_t s;
    explicit Aleatorio(uint64_t semilla) : s(semilla) {}
    uint64_t siguiente() {   // splitmix64
        uint64_t z = (s += 0x9E3779B97F4A7C15ULL);
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
        z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
        return z ^ (z >> 31);
    }
    template <class T>
    void barajar(std::vector<T>& v) {
        for (size_t i = v.size(); i > 1; --i) std::swap(v[i - 1], v[siguiente() % i]);
    }
};

// Indices de w ordenados de mayor a menor; los empates conservan el orden (como sorted de Python).
std::vector<int> orden_decreciente(const std::vector<i64>& w) {
    std::vector<int> orden(w.size());
    std::iota(orden.begin(), orden.end(), 0);
    std::stable_sort(orden.begin(), orden.end(), [&](int a, int b) { return w[a] > w[b]; });
    return orden;
}

// Bitset minimo para la suma de subconjuntos del llenado.
struct Bits {
    std::vector<uint64_t> p;
    explicit Bits(size_t n) : p((n + 63) / 64, 0) {}
    bool get(size_t i) const { return (p[i >> 6] >> (i & 63)) & 1; }
    void set(size_t i) { p[i >> 6] |= uint64_t(1) << (i & 63); }
};

}  // namespace

// ---------------------------------------------------------------------------
// Cotas simples y heuristicas (modelo.py)

i64 cota_l1(const std::vector<i64>& w, i64 C) {
    i64 suma = 0;
    for (i64 x : w) suma += x;
    return suma <= 0 ? 0 : (suma + C - 1) / C;
}

i64 cota_l2(const std::vector<i64>& w, i64 C) {
    // Cota L2 de Martello y Toth (1990). Mismas particiones que modelo.py, en enteros.
    i64 mejor = cota_l1(w, C);
    std::vector<i64> ws(w);
    std::sort(ws.begin(), ws.end());
    std::vector<i64> candidatos{0};
    for (i64 x : ws)
        if (2 * x <= C) candidatos.push_back(x);
    std::sort(candidatos.begin(), candidatos.end());
    candidatos.erase(std::unique(candidatos.begin(), candidatos.end()), candidatos.end());
    for (i64 a : candidatos) {
        i64 n1 = 0, n2 = 0, s2 = 0, s3 = 0;
        for (i64 x : ws) {
            if (x > C - a) n1++;                         // no comparte barra con nada >= a
            else if (2 * x > C) { n2++; s2 += x; }       // una por barra
            else if (x >= a) s3 += x;
        }
        i64 libre = n2 * C - s2;
        i64 falta = s3 - libre;
        i64 extra = falta > 0 ? (falta + C - 1) / C : 0;
        mejor = std::max(mejor, n1 + n2 + extra);
    }
    return mejor;
}

std::vector<std::vector<int>> ffd(const std::vector<i64>& w, i64 C) {
    std::vector<std::vector<int>> barras;
    std::vector<i64> resto;
    for (int i : orden_decreciente(w)) {
        size_t b = 0;
        while (b < barras.size() && resto[b] < w[i]) b++;
        if (b == barras.size()) {
            barras.push_back({i});
            resto.push_back(C - w[i]);
        } else {
            barras[b].push_back(i);
            resto[b] -= w[i];
        }
    }
    return barras;
}

std::vector<std::vector<int>> bfd(const std::vector<i64>& w, i64 C) {
    std::vector<std::vector<int>> barras;
    std::vector<i64> resto;
    for (int i : orden_decreciente(w)) {
        int mb = -1;
        for (size_t b = 0; b < barras.size(); b++)
            if (resto[b] >= w[i] && (mb < 0 || resto[b] < resto[mb])) mb = (int)b;
        if (mb < 0) {
            barras.push_back({i});
            resto.push_back(C - w[i]);
        } else {
            barras[mb].push_back(i);
            resto[mb] -= w[i];
        }
    }
    return barras;
}

std::vector<std::vector<int>> llenado(const std::vector<i64>& w, i64 C) {
    // Una barra a la vez: la pieza pendiente mas larga va fija y se completa con
    // el subconjunto que deja menos hueco (suma de subconjuntos exacta).
    // Para cada suma c se guarda la primera pieza que la alcanza, como en modelo.py.
    std::vector<int> pend = orden_decreciente(w);
    std::vector<std::vector<int>> barras;
    std::vector<int> quien;
    while (!pend.empty()) {
        int fijo = pend[0];
        i64 resto = C - w[fijo];
        size_t n = (size_t)resto + 1;
        Bits alcanza(n), nuevo(n);
        quien.assign(n, -1);
        alcanza.set(0);
        size_t palabras = alcanza.p.size();
        uint64_t mascara = (n % 64) ? ((uint64_t(1) << (n % 64)) - 1) : ~uint64_t(0);
        for (size_t k = 1; k < pend.size(); k++) {
            i64 p = w[pend[k]];
            if (p > resto) continue;
            // nuevo = (alcanza << p) & ~alcanza
            size_t ws = (size_t)p >> 6, bs = (size_t)p & 63;
            for (size_t q = palabras; q-- > 0;) {
                uint64_t v = 0;
                if (q >= ws) {
                    v = alcanza.p[q - ws] << bs;
                    if (bs && q >= ws + 1) v |= alcanza.p[q - ws - 1] >> (64 - bs);
                }
                nuevo.p[q] = v & ~alcanza.p[q];
            }
            nuevo.p[palabras - 1] &= mascara;
            for (size_t q = 0; q < palabras; q++) {
                uint64_t v = nuevo.p[q];
                if (!v) continue;
                alcanza.p[q] |= v;
                while (v) {
                    int b = __builtin_ctzll(v);
                    quien[q * 64 + b] = (int)k;
                    v &= v - 1;
                }
            }
        }
        i64 c = resto;
        while (c > 0 && !alcanza.get((size_t)c)) c--;
        std::vector<int> elegido{fijo};
        std::vector<char> usado(pend.size(), 0);
        usado[0] = 1;
        while (c > 0) {
            int k = quien[(size_t)c];
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

// ---------------------------------------------------------------------------
// Mochila acotada entera (programacion dinamica sobre la capacidad, demanda en
// paquetes 1, 2, 4...). Devuelve el valor y deja el patron en `patron`.

double mochila(const std::vector<double>& valores, const std::vector<i64>& pesos,
               const std::vector<int>& tope, i64 cap, std::vector<int>& patron) {
    const size_t m = pesos.size(), n = (size_t)cap + 1, palabras = (n + 63) / 64;
    std::vector<double> mejor(n, 0.0);
    struct Paquete { int tipo; int copias; i64 peso; size_t desde; };
    std::vector<Paquete> eleccion;
    std::vector<uint64_t> tomar;   // un bitset de n por paquete
    for (size_t j = 0; j < m; j++) {
        int resto = tope[j], k = 1;
        while (resto > 0) {
            int c = std::min(k, resto);
            resto -= c;
            k *= 2;
            i64 p = pesos[j] * c;
            double v = valores[j] * c;
            if (p > cap || v <= 1e-12) continue;
            size_t desde = tomar.size();
            tomar.resize(desde + palabras, 0);
            uint64_t* t = tomar.data() + desde;
            // de mayor a menor capacidad: mejor[c - p] todavia es el de antes de este paquete
            for (size_t q = n; q-- > (size_t)p;) {
                double cand = mejor[q - (size_t)p] + v;
                if (cand > mejor[q] + 1e-12) {
                    mejor[q] = cand;
                    t[q >> 6] |= uint64_t(1) << (q & 63);
                }
            }
            eleccion.push_back({(int)j, c, p, desde});
        }
    }
    size_t c = 0;
    for (size_t q = 1; q < n; q++)
        if (mejor[q] > mejor[c]) c = q;   // primer maximo, como np.argmax
    double valor = mejor[c];
    patron.assign(m, 0);
    for (size_t e = eleccion.size(); e-- > 0;) {
        const Paquete& pq = eleccion[e];
        if ((tomar[pq.desde + (c >> 6)] >> (c & 63)) & 1) {
            patron[pq.tipo] += pq.copias;
            c -= (size_t)pq.peso;
        }
    }
    return valor;
}

// ---------------------------------------------------------------------------
// Relajacion lineal por generacion de columnas: simplex revisado con la inversa
// de la base explicita (m x m, m = largos distintos).
//
//   min sum(x_p)   sujeto a   sum_p a_jp * x_p >= demanda_j,   x >= 0
//
// Cada posicion de la base es un patron (cuesta 1 barra) o una holgura de
// exceso -e_j (cuesta 0). Tres piezas que se anadieron despues de verlo fallar:
//  - CANTERA: todo patron generado se guarda y se busca ahi uno que mejore antes
//    de llamar a la mochila (lo caro).
//  - ANTICICLADO: la demanda se perturba (~1e-6, distinto por largo) solo para
//    elegir quien sale de la base. Sin eso el simplex cicla.
//  - SEMILLA: quien llama mete en la cantera los patrones de las heuristicas.
// La cota sale de Farley: (demanda . precios) / (valor del mejor patron), valida
// en cualquier vuelta aunque el LP no converja.

ResultadoLP lp_patrones(const std::vector<i64>& pesos, const std::vector<int>& dem, i64 cap,
                        Cantera* cantera, const Control* control, long long max_iter, bool diagnostico) {
    const int m = (int)pesos.size();
    ResultadoLP r;
    std::vector<double> d(m), dp(m);
    for (int j = 0; j < m; j++) {
        d[j] = dem[j];
        double t = 0.6180339887498949 * (j + 1);
        dp[j] = d[j] + EPS_PERTURBA * (0.5 + (t - std::floor(t)));
    }
    std::vector<std::vector<int>> base(m);    // patron, o vacio si la posicion es una holgura
    std::vector<int> holgura(m, -1);          // holgura[i] = j si la posicion i es la holgura del largo j
    std::vector<double> costo(m, 1.0), inv((size_t)m * m, 0.0);
    std::vector<std::vector<int>> cols;       // la cantera, en el orden de `pesos`
    std::set<std::vector<int>> vistos;
    std::vector<double> P;                    // la misma cantera como matriz (fila por patron)

    auto guardar = [&](const std::vector<int>& pat) {
        bool alguno = false;
        for (int a : pat) alguno |= (a != 0);
        if (!alguno || !vistos.insert(pat).second) return;
        cols.push_back(pat);
        for (int a : pat) P.push_back((double)a);
    };

    for (int j = 0; j < m; j++) {             // base inicial: un patron homogeneo por largo
        std::vector<int> a(m, 0);
        a[j] = (int)std::min<i64>(dem[j], cap / pesos[j]);
        base[j] = a;
        inv[(size_t)j * m + j] = 1.0 / a[j];
        guardar(a);
    }
    if (cantera)                              // patrones heredados, recortados a la demanda de ahora
        for (const auto& pd : *cantera) {
            std::vector<int> a(m, 0);
            for (int j = 0; j < m; j++) {
                auto it = pd.find(pesos[j]);
                a[j] = it == pd.end() ? 0 : std::min(it->second, dem[j]);
            }
            guardar(a);
        }

    std::vector<double> x(m), y(m), yp(m), u(m), col(m), fila(m);
    std::vector<int> entra_pat;
    double farley = 0.0;
    long long it = 0;
    for (it = 1; it <= max_iter; it++) {
        if (control && control->vencido()) {
            r.interrumpido = true;
            break;
        }
        // x = inv * demanda perturbada (se recalcula entero: no acumula error)
        for (int i = 0; i < m; i++) {
            double s = 0;
            const double* fi = &inv[(size_t)i * m];
            for (int k = 0; k < m; k++) s += fi[k] * dp[k];
            x[i] = std::max(s, 0.0);
        }
        // y = costo * inv: precio de cada largo
        std::fill(y.begin(), y.end(), 0.0);
        for (int i = 0; i < m; i++) {
            if (costo[i] == 0.0) continue;
            const double* fi = &inv[(size_t)i * m];
            for (int k = 0; k < m; k++) y[k] += costo[i] * fi[k];
        }
        int jmin = 0;
        for (int k = 1; k < m; k++)
            if (y[k] < y[jmin]) jmin = k;
        int j_holgura;
        double c_entra;
        bool es_holgura;
        if (y[jmin] < -EPS_PRECIO) {
            // un precio negativo: entra la holgura de ese largo (columna -e_j, costo 0)
            es_holgura = true;
            j_holgura = jmin;
            c_entra = 0.0;
            std::fill(col.begin(), col.end(), 0.0);
            col[jmin] = -1.0;
        } else {
            for (int k = 0; k < m; k++) yp[k] = std::max(y[k], 0.0);
            // 1) lo barato: algun patron de la cantera mejora?
            size_t kmax = 0;
            double vmax = -std::numeric_limits<double>::infinity();
            for (size_t c = 0; c < cols.size(); c++) {
                const double* pc = &P[c * m];
                double v = 0;
                for (int k = 0; k < m; k++) v += pc[k] * yp[k];
                if (v > vmax) { vmax = v; kmax = c; }
            }
            if (vmax > 1.0 + EPS_COSTO) {
                entra_pat = cols[kmax];
            } else {
                // 2) lo caro: la mochila busca entre TODOS los patrones
                r.mochilas++;
                double valor = mochila(yp, pesos, dem, cap, entra_pat);
                double dy = 0;
                for (int k = 0; k < m; k++) dy += d[k] * yp[k];
                farley = std::max(farley, dy / std::max(valor, 1.0));
                if (valor <= 1.0 + EPS_COSTO) {
                    r.convergio = true;
                    break;
                }
                guardar(entra_pat);
            }
            es_holgura = false;
            j_holgura = -1;
            c_entra = 1.0;
            for (int k = 0; k < m; k++) col[k] = entra_pat[k];
        }
        // u = inv * columna que entra; prueba de la razon: sale quien antes llega a cero
        int sale = -1;
        double mejor_razon = std::numeric_limits<double>::infinity();
        for (int i = 0; i < m; i++) {
            double s = 0;
            const double* fi = &inv[(size_t)i * m];
            for (int k = 0; k < m; k++) s += fi[k] * col[k];
            u[i] = s;
            if (s > EPS_PIVOTE) {
                double razon = x[i] / s;
                if (sale < 0 || razon < mejor_razon) { sale = i; mejor_razon = razon; }   // empate -> indice menor
            }
        }
        if (sale < 0) throw std::runtime_error("LP no acotado: no deberia pasar en este problema");
        const double* fs = &inv[(size_t)sale * m];
        for (int k = 0; k < m; k++) fila[k] = fs[k] / u[sale];
        for (int i = 0; i < m; i++) {
            if (i == sale) continue;
            double ui = u[i];
            if (ui == 0.0) continue;
            double* fi = &inv[(size_t)i * m];
            for (int k = 0; k < m; k++) fi[k] -= ui * fila[k];
        }
        std::copy(fila.begin(), fila.end(), inv.begin() + (size_t)sale * m);
        if (es_holgura) base[sale].clear();
        else base[sale] = entra_pat;
        holgura[sale] = j_holgura;
        costo[sale] = c_entra;
    }
    r.vueltas = std::min(it, max_iter);
    if (diagnostico) {   // cuanto se desvio la inversa de la base: max |B * inv - I|
        double err = 0;
        for (int i = 0; i < m; i++)
            for (int k = 0; k < m; k++) {
                double s = 0;
                for (int c = 0; c < m; c++) {
                    double b = base[c].empty() ? (holgura[c] == i ? -1.0 : 0.0) : (double)base[c][i];
                    s += b * inv[(size_t)c * m + k];
                }
                err = std::max(err, std::fabs(s - (i == k ? 1.0 : 0.0)));
            }
        r.error_inversa = err;
    }
    if (cantera) {   // devolver la cantera ampliada, en forma peso -> cantidad
        cantera->clear();
        for (const auto& c : cols) {
            std::map<i64, int> pd;
            for (int j = 0; j < m; j++)
                if (c[j]) pd[pesos[j]] = c[j];
            cantera->push_back(std::move(pd));
        }
    }
    r.cota = (i64)std::ceil(farley - 1e-6);   // 1e-6 >> error de redondeo: nunca redondea de mas
    if (r.cota < 0) r.cota = 0;
    for (int i = 0; i < m; i++) {             // la solucion, con la demanda real
        if (base[i].empty()) continue;
        double s = 0;
        const double* fi = &inv[(size_t)i * m];
        for (int k = 0; k < m; k++) s += fi[k] * d[k];
        r.patrones.push_back(base[i]);
        r.veces.push_back(std::max(s, 0.0));
    }
    return r;
}

// ---------------------------------------------------------------------------
// El metodo completo (simplex_propio.resolver)

namespace {

// Las tres heuristicas simples sobre lo pendiente, como barras de pesos.
std::vector<Barras> simples(const std::vector<i64>& pesos, const std::vector<int>& dem, i64 cap) {
    std::vector<i64> resto;
    for (size_t j = 0; j < pesos.size(); j++)
        for (int k = 0; k < dem[j]; k++) resto.push_back(pesos[j]);
    std::vector<Barras> sols;
    for (auto&& sol : {ffd(resto, cap), bfd(resto, cap), llenado(resto, cap)}) {
        Barras b;
        for (const auto& barra : sol) {
            std::vector<i64> pb;
            for (int i : barra) pb.push_back(resto[i]);
            b.push_back(std::move(pb));
        }
        sols.push_back(std::move(b));
    }
    return sols;
}

struct Pasada {
    Barras mejor;
    bool hay_mejor = false;
    bool hay_cota = false;
    i64 cota = 0;
    bool convergio = true;
};

// Una pasada completa con los largos en el orden dado.
Pasada una_pasada(const std::vector<i64>& pesos, std::vector<int> dem, i64 cap, Info& info,
                  const Control* control, bool clavar = true, int max_pasos = 100000) {
    Pasada r;
    Barras hechas;     // barras ya decididas
    Cantera cantera;
    const int nt = (int)pesos.size();
    auto poner = [&](const std::vector<int>& vivos, const std::vector<i64>& ps, const std::vector<int>& usa) {
        std::vector<i64> barra;
        for (size_t i = 0; i < usa.size(); i++) {
            dem[vivos[i]] -= usa[i];
            for (int a = 0; a < usa[i]; a++) barra.push_back(ps[i]);
        }
        hechas.push_back(std::move(barra));
    };
    for (int paso = 0; paso < max_pasos; paso++) {
        std::vector<int> vivos;
        for (int j = 0; j < nt; j++)
            if (dem[j] > 0) vivos.push_back(j);
        if (vivos.empty()) {
            if (!r.hay_mejor || hechas.size() < r.mejor.size()) {
                r.mejor = hechas;
                r.hay_mejor = true;
            }
            break;
        }
        std::vector<i64> ps;
        std::vector<int> ds;
        for (int j : vivos) {
            ps.push_back(pesos[j]);
            ds.push_back(dem[j]);
        }
        // candidato: lo decidido hasta aqui + cierre con heuristicas simples
        std::vector<Barras> cierres = simples(ps, ds, cap);
        size_t kc = 0;
        for (size_t k = 1; k < cierres.size(); k++)
            if (cierres[k].size() < cierres[kc].size()) kc = k;
        if (!r.hay_mejor || hechas.size() + cierres[kc].size() < r.mejor.size()) {
            r.mejor = hechas;
            r.mejor.insert(r.mejor.end(), cierres[kc].begin(), cierres[kc].end());
            r.hay_mejor = true;
        }
        if (r.hay_cota && (i64)r.mejor.size() == r.cota) break;   // igualo la cota: es el minimo
        if (control && control->vencido()) {
            info.interrumpido = true;
            break;
        }
        for (const auto& sol : cierres)   // los patrones de las heuristicas adelantan al LP
            for (const auto& barra : sol) {
                std::map<i64, int> pd;
                for (i64 p : barra) pd[p]++;
                cantera.push_back(std::move(pd));
            }
        ResultadoLP lp = lp_patrones(ps, ds, cap, &cantera, control);
        info.n_lp++;
        info.vueltas += lp.vueltas;
        info.mochilas += lp.mochilas;
        if (!r.hay_cota) {
            r.hay_cota = true;
            r.cota = lp.cota;
            r.convergio = lp.convergio;
        }
        if (lp.interrumpido) {
            info.interrumpido = true;
            break;
        }
        if ((i64)r.mejor.size() == r.cota) break;
        if ((i64)hechas.size() + lp.cota >= (i64)r.mejor.size()) break;   // por aqui ya no se mejora
        int avance = 0;
        for (size_t b = 0; b < lp.patrones.size(); b++) {
            long long veces = (long long)std::floor(lp.veces[b] + 1e-9);
            for (long long t = 0; t < veces; t++) {
                std::vector<int> usa(vivos.size());
                int suma = 0;
                for (size_t i = 0; i < vivos.size(); i++) {
                    usa[i] = std::min(lp.patrones[b][i], dem[vivos[i]]);   // nunca producir de mas
                    suma += usa[i];
                }
                if (suma == 0) break;
                poner(vivos, ps, usa);
                avance++;
            }
        }
        if (avance) continue;
        if (!clavar) break;
        // nada entero: clavar el patron mas usado; empate -> el que llena mas la barra
        size_t k = 0;
        long long mejor_x = 0;
        i64 mejor_lleno = 0;
        for (size_t b = 0; b < lp.patrones.size(); b++) {
            long long xr = std::llround(lp.veces[b] * 1e9);
            i64 lleno = 0;
            for (size_t i = 0; i < ps.size(); i++) lleno += lp.patrones[b][i] * ps[i];
            if (b == 0 || xr > mejor_x || (xr == mejor_x && lleno > mejor_lleno)) {
                k = b;
                mejor_x = xr;
                mejor_lleno = lleno;
            }
        }
        if (lp.patrones.empty()) break;
        std::vector<int> usa(vivos.size());
        int suma = 0;
        for (size_t i = 0; i < vivos.size(); i++) {
            usa[i] = std::min(lp.patrones[k][i], dem[vivos[i]]);
            suma += usa[i];
        }
        if (suma == 0) break;
        poner(vivos, ps, usa);
    }
    return r;
}

}  // namespace

ResultadoPesos resolver_pesos(const std::vector<i64>& w, i64 C, int arranques, const Control* control) {
    ResultadoPesos res;
    if (C <= 0) throw std::invalid_argument("la capacidad de la barra debe ser positiva");
    std::map<i64, int, std::greater<i64>> cuenta;
    for (i64 x : w) {
        if (x <= 0) throw std::invalid_argument("hay una pieza de largo cero o negativo");
        if (x > C) throw std::invalid_argument("hay una pieza que no cabe en la barra: quien llama debe apartarla antes");
        cuenta[x]++;
    }
    if (w.empty()) return res;
    std::vector<i64> pesos;
    for (const auto& kv : cuenta) pesos.push_back(kv.first);   // de mayor a menor
    Aleatorio rng(0x4E657374547562ULL);
    bool hay_cota = false;
    if (arranques < 1) arranques = 1;
    for (int a = 0; a < arranques; a++) {
        std::vector<int> dem;
        for (i64 p : pesos) dem.push_back(cuenta[p]);
        Pasada ps = una_pasada(pesos, dem, C, res.info, control);
        res.info.arranques++;
        res.info.convergio = res.info.convergio && ps.convergio;
        if (ps.hay_cota) {   // toda cota es valida: vale la mayor
            res.cota_lp = hay_cota ? std::max(res.cota_lp, ps.cota) : ps.cota;
            hay_cota = true;
        }
        if (ps.hay_mejor && (a == 0 || ps.mejor.size() < res.barras.size())) res.barras = std::move(ps.mejor);
        if (hay_cota && (i64)res.barras.size() == res.cota_lp) break;
        if (res.info.interrumpido) break;
        rng.barajar(pesos);
    }
    res.cota = std::max(res.cota_lp, cota_l2(w, C));
    return res;
}

bool validar_pesos(const std::vector<i64>& w, i64 C, const Barras& barras, std::string* motivo) {
    auto fallo = [&](const std::string& m) {
        if (motivo) *motivo = m;
        return false;
    };
    std::map<i64, long long> pedido, puesto;
    for (i64 x : w) pedido[x]++;
    for (size_t k = 0; k < barras.size(); k++) {
        if (barras[k].empty()) return fallo("barra " + std::to_string(k + 1) + " vacia");
        i64 suma = 0;
        for (i64 x : barras[k]) {
            puesto[x]++;
            suma += x;
        }
        if (suma > C)
            return fallo("barra " + std::to_string(k + 1) + " pasada: " + std::to_string(suma) + " > " + std::to_string(C));
    }
    if (pedido != puesto) return fallo("las piezas colocadas no son las pedidas");
    if (motivo) *motivo = "ok";
    return true;
}

// ---------------------------------------------------------------------------
// Nivel 2: un perfil

double Plan::aprovechamiento() const {
    if (barras.empty() || parametros.largo_barra <= 0) return 0.0;
    return (double)largo_colocado / ((double)barras.size() * (double)parametros.largo_barra);
}

Plan calcular_perfil(const std::vector<Pieza>& piezas, const Parametros& p, int arranques, const Control* control) {
    if (p.largo_barra <= 0) throw std::invalid_argument("el largo de barra debe ser positivo");
    if (p.despunte < 0 || p.zona_muerta < 0 || p.separacion < 0)
        throw std::invalid_argument("despunte, zona muerta y separacion no pueden ser negativos");
    const i64 util = p.util();
    if (util <= 0) throw std::invalid_argument("despunte y zona muerta se comen toda la barra");
    Plan plan;
    plan.parametros = p;
    // ids por largo, en el orden de entrada (para repartirlos entre las barras)
    std::map<i64, std::deque<int>> ids_por_largo;
    std::vector<i64> w;
    for (const Pieza& pz : piezas) {
        if (pz.largo <= 0) throw std::invalid_argument("hay una pieza de largo cero o negativo");
        if (pz.cantidad <= 0) throw std::invalid_argument("hay una pieza con cantidad cero o negativa");
        if (pz.largo > util) {
            plan.no_caben.push_back(pz.id);
            continue;
        }
        for (int k = 0; k < pz.cantidad; k++) {
            ids_por_largo[pz.largo].push_back(pz.id);
            w.push_back(pz.largo + p.separacion);
        }
    }
    const i64 C = util + p.separacion;
    ResultadoPesos r = resolver_pesos(w, C, arranques, control);
    plan.info = r.info;
    plan.cota = r.cota;
    plan.cota_lp = r.cota_lp;
    // barras como largos de mayor a menor; barras iguales quedan juntas
    std::vector<std::vector<i64>> largos;
    for (const auto& b : r.barras) {
        std::vector<i64> l;
        for (i64 x : b) l.push_back(x - p.separacion);
        std::sort(l.begin(), l.end(), std::greater<i64>());
        largos.push_back(std::move(l));
    }
    std::stable_sort(largos.begin(), largos.end(), std::greater<std::vector<i64>>());
    for (const auto& l : largos) {
        BarraPlan bp;
        i64 pos = p.despunte;
        for (size_t i = 0; i < l.size(); i++) {
            if (i) pos += p.separacion;
            Colocada c;
            auto& cola = ids_por_largo[l[i]];
            c.id = cola.front();
            cola.pop_front();
            c.largo = l[i];
            c.inicio = pos;
            c.fin = pos + l[i];
            pos = c.fin;
            bp.piezas.push_back(c);
            plan.piezas_colocadas++;
            plan.largo_colocado += l[i];
        }
        bp.ocupado = pos - p.despunte;
        bp.libre = util - bp.ocupado;
        plan.barras.push_back(std::move(bp));
    }
    plan.demostrado = (i64)plan.barras.size() == plan.cota;
    if (r.info.interrumpido)
        plan.estado = (control && control->cancelado()) ? Estado::cancelado : Estado::tiempo_agotado;
    return plan;
}

bool validar_plan(const std::vector<Pieza>& piezas, const Parametros& p, const Plan& plan, std::string* motivo) {
    // Escrito aparte del calculo: solo mira la entrada y el plan, en unidades reales.
    auto fallo = [&](const std::string& m) {
        if (motivo) *motivo = m;
        return false;
    };
    const i64 util = p.largo_barra - p.despunte - p.zona_muerta;
    std::map<int, i64> largo_de;
    std::map<int, long long> pedido, puesto;
    std::set<int> fuera;
    for (const Pieza& pz : piezas) {
        if (largo_de.count(pz.id)) return fallo("id de pieza repetido: " + std::to_string(pz.id));
        largo_de[pz.id] = pz.largo;
        if (pz.largo > util) fuera.insert(pz.id);
        else pedido[pz.id] += pz.cantidad;
    }
    std::set<int> no_caben(plan.no_caben.begin(), plan.no_caben.end());
    if (no_caben != fuera || no_caben.size() != plan.no_caben.size())
        return fallo("la lista de piezas que no caben no es la correcta");
    for (size_t k = 0; k < plan.barras.size(); k++) {
        const auto& b = plan.barras[k];
        std::string nb = "barra " + std::to_string(k + 1);
        if (b.piezas.empty()) return fallo(nb + " vacia");
        i64 fin_anterior = 0;
        for (size_t i = 0; i < b.piezas.size(); i++) {
            const Colocada& c = b.piezas[i];
            auto it = largo_de.find(c.id);
            if (it == largo_de.end()) return fallo(nb + ": pieza que no se pidio");
            if (fuera.count(c.id)) return fallo(nb + ": pieza que no cabe");
            if (c.largo != it->second || c.fin - c.inicio != c.largo) return fallo(nb + ": largo cambiado");
            i64 esperado = i == 0 ? p.despunte : fin_anterior + p.separacion;
            if (c.inicio < esperado) return fallo(nb + ": piezas montadas o sin despunte");
            fin_anterior = c.fin;
            puesto[c.id]++;
        }
        if (fin_anterior > p.largo_barra - p.zona_muerta) return fallo(nb + " pasada: entra en la zona muerta");
        i64 ocupado = fin_anterior - p.despunte;
        if (b.ocupado != ocupado || b.libre != util - ocupado) return fallo(nb + ": ocupado o libre mal calculado");
    }
    if (pedido != puesto) return fallo("las piezas colocadas no son las pedidas");
    if (plan.cota > (i64)plan.barras.size()) return fallo("la cota supera las barras: imposible");
    if (motivo) *motivo = "ok";
    return true;
}

std::vector<Grupo> agrupar(const Plan& plan) {
    std::vector<Grupo> g;
    auto iguales = [](const BarraPlan& a, const BarraPlan& b) {
        if (a.piezas.size() != b.piezas.size()) return false;
        for (size_t i = 0; i < a.piezas.size(); i++)
            if (a.piezas[i].id != b.piezas[i].id || a.piezas[i].largo != b.piezas[i].largo) return false;
        return true;
    };
    for (size_t k = 0; k < plan.barras.size(); k++) {
        if (!g.empty() && iguales(plan.barras[g.back().primera], plan.barras[k])) g.back().veces++;
        else g.push_back({(int)k, 1});
    }
    return g;
}

bool leer_decimas(const std::string& texto, i64& decimas) {
    size_t i = 0, n = texto.size();
    while (i < n && (texto[i] == ' ' || texto[i] == '\t')) i++;
    while (n > i && (texto[n - 1] == ' ' || texto[n - 1] == '\t')) n--;
    if (i == n) return false;
    i64 entero = 0;
    int digitos = 0, dec = -1;
    bool hay_sep = false;
    for (; i < n; i++) {
        char c = texto[i];
        if (c >= '0' && c <= '9') {
            if (hay_sep) {
                if (dec >= 0) return false;   // mas de un decimal
                dec = c - '0';
            } else {
                if (++digitos > 12) return false;
                entero = entero * 10 + (c - '0');
            }
        } else if ((c == ',' || c == '.') && !hay_sep) {
            hay_sep = true;
        } else {
            return false;
        }
    }
    if (digitos == 0 && dec < 0) return false;
    decimas = entero * 10 + (dec < 0 ? 0 : dec);
    return true;
}

}  // namespace nt
