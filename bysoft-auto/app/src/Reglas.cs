// Reglas de negocio puras (sin BySoft): interpretar subcarpetas, tabla de
// materiales, eleccion de .PAR (espesor redondeado hacia arriba) y lectura del
// Excel de cantidades. Se pueden probar sin BySoft instalado.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;

namespace AutoBySoft
{
    public sealed class ParInfo
    {
        public string Archivo;      // nombre del archivo .PAR (sin carpeta)
        public string Material;     // DC01, DD11, 1.4301, AW5754...
        public double Espesor;
        public string Gas;          // O2 / N2
        public string Sufijo;       // "" si no tiene (QUALITY, SPEED...)
    }

    public sealed class Familia
    {
        public string Nombre;                                   // ACERO, INOX...
        public Dictionary<string, string> MaterialParABySoft;   // DC01 -> DC01 ; 1.4301 -> 1.4031
        public string Gas;
        public string ReglaDeCorte;                             // ByFiber_O2
        public List<string> Sufijos;                            // orden de preferencia; "" = sin sufijo
    }

    public sealed class Pieza
    {
        public string Referencia;
        public int Cantidad;
        public string Observacion = "";
        public string RutaDxf;
        public List<string> UbicacionesEnBySoft = new List<string>();
        public bool Importar = true;
        public bool Sobrescribir;           // actualizar una pieza que ya existe (afecta nesteos anteriores)
        public bool Conservar;              // renombrar la version anterior y luego importar la nueva
        public string NombreAnterior;       // nombre que recibio la version anterior (si Conservar)
        public string CarpetaLocal;         // carpeta BySoft donde se importa ("/A/B/")
    }

    public sealed class Programa
    {
        public string Subcarpeta;           // nombre de la subcarpeta del pedido
        public string RutaSubcarpeta;
        public string MaterialTexto;        // "SAEJ 050"
        public bool MaterialAsumido;        // la subcarpeta no decia material
        public Familia Familia;
        public double EspesorReal;
        public ParInfo Par;
        public string MaterialBySoft;       // DC01, DD11...
        public string Nombre;               // LP0926732
        public int Consecutivo;
        public string DestinoRelativo;      // DESARROLLO\150. X\SAEJ 050 Esp=3.42mm
        public List<Pieza> Piezas = new List<Pieza>();
        public List<string> DxfSinCantidad = new List<string>();
    }

    public static class Reglas
    {
        private static readonly CultureInfo Inv = CultureInfo.InvariantCulture;

        // ---------------------------------------------------------------- subcarpetas
        private static readonly Regex RxEspIgual = new Regex(@"(?:ESP|E)\s*[=:]\s*(\d+(?:[.,]\d+)?)\s*(?:MM)?", RegexOptions.IgnoreCase);
        private static readonly Regex RxNumMm = new Regex(@"(\d+(?:[.,]\d+)?)\s*MM\b", RegexOptions.IgnoreCase);
        private static readonly Regex RxNumInicio = new Regex(@"^\s*(\d+(?:[.,]\d+)?)(?=\s|$|[;_-])", RegexOptions.IgnoreCase);

        // "SAEJ 050 Esp=3.42mm" -> ("SAEJ 050", 3.42). Devuelve false si no hay espesor.
        public static bool InterpretarSubcarpeta(string nombre, out string material, out double espesor)
        {
            material = "";
            espesor = 0;
            Match m = RxEspIgual.Match(nombre);
            if (!m.Success)
            {
                m = RxNumMm.Match(nombre);
            }
            if (!m.Success)
            {
                m = RxNumInicio.Match(nombre);
            }
            if (!m.Success)
            {
                return false;
            }
            if (!double.TryParse(m.Groups[1].Value.Replace(',', '.'), NumberStyles.Float, Inv, out espesor) || espesor <= 0)
            {
                return false;
            }
            string resto = nombre.Remove(m.Index, m.Length);
            resto = Regex.Replace(resto, @"\b(ESP|E)\b\s*[=:]?", " ", RegexOptions.IgnoreCase);
            resto = Regex.Replace(resto, @"[;=_]+", " ");
            material = Regex.Replace(resto, @"\s+", " ").Trim(' ', '-', ',', '.');
            return true;
        }

        public static string Normalizar(string s)
        {
            StringBuilder sb = new StringBuilder();
            foreach (char ch in (s ?? "").ToUpperInvariant())
            {
                if (char.IsLetterOrDigit(ch) || ch == '&' || ch == '+')
                {
                    sb.Append(ch);
                }
            }
            return sb.ToString();
        }

        // ---------------------------------------------------------------- configuracion
        // materiales.txt: lineas "ALIAS;FAMILIA". El orden importa: la primera que coincide gana.
        public static List<KeyValuePair<string, string>> LeerAlias(string ruta)
        {
            List<KeyValuePair<string, string>> lista = new List<KeyValuePair<string, string>>();
            foreach (string linea in LineasUtiles(ruta))
            {
                string[] p = linea.Split(';');
                if (p.Length >= 2 && Normalizar(p[0]).Length > 0)
                {
                    lista.Add(new KeyValuePair<string, string>(Normalizar(p[0]), p[1].Trim().ToUpperInvariant()));
                }
            }
            return lista;
        }

        // tecnologia.txt: "FAMILIA;MATPAR=MATBYSOFT,...;GAS;REGLA;SUFIJOS(separados por |, vacio = sin sufijo)"
        public static Dictionary<string, Familia> LeerFamilias(string ruta)
        {
            Dictionary<string, Familia> d = new Dictionary<string, Familia>(StringComparer.OrdinalIgnoreCase);
            foreach (string linea in LineasUtiles(ruta))
            {
                string[] p = linea.Split(';');
                if (p.Length < 4)
                {
                    continue;
                }
                Familia f = new Familia();
                f.Nombre = p[0].Trim().ToUpperInvariant();
                f.MaterialParABySoft = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
                foreach (string par in p[1].Split(','))
                {
                    string[] kv = par.Split('=');
                    if (kv.Length == 2 && kv[0].Trim().Length > 0)
                    {
                        f.MaterialParABySoft[kv[0].Trim()] = kv[1].Trim();
                    }
                }
                f.Gas = p[2].Trim().ToUpperInvariant();
                f.ReglaDeCorte = p[3].Trim();
                f.Sufijos = new List<string>();
                string sufijos = p.Length >= 5 ? p[4] : "";
                foreach (string s in sufijos.Split('|'))
                {
                    f.Sufijos.Add(s.Trim().ToUpperInvariant());
                }
                if (f.Sufijos.Count == 0)
                {
                    f.Sufijos.Add("");
                }
                d[f.Nombre] = f;
            }
            return d;
        }

        private static IEnumerable<string> LineasUtiles(string ruta)
        {
            foreach (string l in File.ReadAllLines(ruta, Encoding.UTF8))
            {
                string t = l.Trim();
                if (t.Length > 0 && !t.StartsWith("#"))
                {
                    yield return t;
                }
            }
        }

        // Familia de un texto de material (por alias). null si no hay coincidencia.
        public static string FamiliaDe(string materialTexto, List<KeyValuePair<string, string>> alias)
        {
            string n = Normalizar(materialTexto);
            if (n.Length == 0)
            {
                return null;
            }
            foreach (KeyValuePair<string, string> a in alias)
            {
                if (n.Contains(a.Key))
                {
                    return a.Value;
                }
            }
            return null;
        }

        // ---------------------------------------------------------------- .PAR
        private static readonly Regex RxPar = new Regex(
            @"^.+?_6000_(?<mat>.+?)_(?<esp>\d+(?:\.\d+)?)_\d{3}_(?<gas>N2|O2)(?:_(?<suf>[^.]+))?\.PAR$",
            RegexOptions.IgnoreCase);

        public static ParInfo InterpretarPar(string archivo)
        {
            Match m = RxPar.Match(archivo);
            if (!m.Success)
            {
                return null;
            }
            ParInfo p = new ParInfo();
            p.Archivo = archivo;
            p.Material = m.Groups["mat"].Value;
            p.Espesor = double.Parse(m.Groups["esp"].Value, Inv);
            p.Gas = m.Groups["gas"].Value.ToUpperInvariant();
            p.Sufijo = m.Groups["suf"].Success ? m.Groups["suf"].Value.ToUpperInvariant() : "";
            return p.Espesor > 0 ? p : null;
        }

        public static List<ParInfo> LeerPars(string carpeta)
        {
            List<ParInfo> l = new List<ParInfo>();
            foreach (string f in Directory.GetFiles(carpeta, "*.PAR", SearchOption.TopDirectoryOnly))
            {
                ParInfo p = InterpretarPar(Path.GetFileName(f));
                if (p != null)
                {
                    l.Add(p);
                }
            }
            return l;
        }

        // Regla del usuario: el espesor inmediatamente SUPERIOR (o igual) que tenga .PAR,
        // entre los materiales y el gas de la familia. Con varios sufijos, el de mayor preferencia.
        public static ParInfo ElegirPar(Familia f, double espesor, List<ParInfo> pars)
        {
            List<ParInfo> candidatos = pars.Where(p =>
                f.MaterialParABySoft.ContainsKey(p.Material) &&
                string.Equals(p.Gas, f.Gas, StringComparison.OrdinalIgnoreCase) &&
                f.Sufijos.Contains(p.Sufijo) &&
                p.Espesor >= espesor - 1e-6).ToList();
            if (candidatos.Count == 0)
            {
                return null;
            }
            double minimo = candidatos.Min(p => p.Espesor);
            return candidatos.Where(p => Math.Abs(p.Espesor - minimo) < 1e-6)
                             .OrderBy(p => f.Sufijos.IndexOf(p.Sufijo))
                             .ThenBy(p => p.Archivo, StringComparer.OrdinalIgnoreCase)
                             .First();
        }

        public static string Num(double d)
        {
            return d.ToString("0.###", Inv);
        }

        // ---------------------------------------------------------------- Excel de cantidades
        // Busca la fila de encabezado (columna A = "Referencia") y lee hasta el final.
        // Devuelve las piezas sumando repetidas; los avisos se agregan a 'avisos'.
        public static List<Pieza> LeerCantidades(List<string[]> filas, List<string> errores, List<string> avisos)
        {
            int inicio = -1;
            for (int i = 0; i < filas.Count && i < 20; i++)
            {
                if (filas[i].Length > 0 && string.Equals(filas[i][0].Trim(), "Referencia", StringComparison.OrdinalIgnoreCase))
                {
                    inicio = i + 1;
                    break;
                }
            }
            if (inicio < 0)
            {
                errores.Add("El Excel de cantidades no tiene el encabezado 'Referencia' en la columna A (usa FORMATO_CANTIDADES.xlsx).");
                return new List<Pieza>();
            }
            Dictionary<string, Pieza> porNombre = new Dictionary<string, Pieza>(StringComparer.OrdinalIgnoreCase);
            List<Pieza> orden = new List<Pieza>();
            for (int i = inicio; i < filas.Count; i++)
            {
                string[] f = filas[i];
                string referencia = f.Length > 0 ? f[0].Trim() : "";
                string cant = f.Length > 1 ? f[1].Trim() : "";
                string obs = f.Length > 2 ? f[2].Trim() : "";
                if (referencia.Length == 0 && cant.Length == 0)
                {
                    continue;
                }
                int fila = i + 1;
                if (referencia.EndsWith(".dxf", StringComparison.OrdinalIgnoreCase))
                {
                    referencia = referencia.Substring(0, referencia.Length - 4);
                }
                if (referencia.Length == 0)
                {
                    errores.Add("Fila " + fila + ": cantidad sin referencia.");
                    continue;
                }
                double dc;
                if (!double.TryParse(cant.Replace(',', '.'), NumberStyles.Float, Inv, out dc) || dc < 1 || Math.Abs(dc - Math.Round(dc)) > 1e-9)
                {
                    errores.Add("Fila " + fila + " (" + referencia + "): la cantidad '" + cant + "' no es un entero >= 1.");
                    continue;
                }
                int c = (int)Math.Round(dc);
                Pieza p;
                if (porNombre.TryGetValue(referencia, out p))
                {
                    p.Cantidad += c;
                    avisos.Add("La referencia " + referencia + " aparece varias veces: se suman las cantidades (total " + p.Cantidad + ").");
                    if (obs.Length > 0 && p.Observacion.IndexOf(obs, StringComparison.OrdinalIgnoreCase) < 0)
                    {
                        p.Observacion = p.Observacion.Length == 0 ? obs : p.Observacion + " / " + obs;
                    }
                }
                else
                {
                    p = new Pieza();
                    p.Referencia = referencia;
                    p.Cantidad = c;
                    p.Observacion = obs;
                    porNombre[referencia] = p;
                    orden.Add(p);
                }
            }
            if (orden.Count == 0 && errores.Count == 0)
            {
                errores.Add("El Excel de cantidades no tiene ninguna referencia.");
            }
            return orden;
        }

        // Nombre de programa: prefijo + MM + AA + consecutivo (ej. LP0926732).
        public static string NombrePrograma(string prefijo, DateTime fecha, int consecutivo)
        {
            return prefijo + fecha.ToString("MM", Inv) + fecha.ToString("yy", Inv) + consecutivo.ToString(Inv);
        }
    }
}
