// Lee el listado de programas (LISTADO_PROGRAMAS_LASER-FANALCA.xls) con Excel
// (COM, sin referencias: el .xls antiguo no se puede leer de otra forma sin librerias)
// para proponer el siguiente consecutivo. Solo lectura: abre el libro en modo lectura.

using System;
using System.Globalization;
using System.Reflection;
using System.Runtime.InteropServices;

namespace AutoBySoft
{
    public static class Listado
    {
        // Hojas y columnas (base 0) del listado.
        //   LASER-PRODUCCION:  B=CODIGO(LP)  E=consecutivo  G=NOMBRE
        //   LASER-DESARROLLO:  B=CODIGO(LD)  E=consecutivo  F=NOMBRE
        public static string Hoja(string prefijo)
        {
            return prefijo == "LP" ? "LASER-PRODUCCION" : "LASER-DESARROLLO";
        }

        public static int ColumnaNombre(string prefijo)
        {
            return prefijo == "LP" ? 6 : 5;
        }

        // Primer consecutivo cuya fila tiene el NOMBRE vacio (filas pre-numeradas).
        // Si no hay filas pre-numeradas: el mayor consecutivo con nombre + 1.
        public static int SiguienteConsecutivo(string rutaXls, string prefijo, out string detalle)
        {
            object[,] datos = LeerHoja(rutaXls, Hoja(prefijo));
            int fil0 = datos.GetLowerBound(0), fil1 = datos.GetUpperBound(0);
            int col0 = datos.GetLowerBound(1);
            int colNombre = col0 + ColumnaNombre(prefijo);
            int colCodigo = col0 + 1, colCons = col0 + 4;
            int maxConNombre = 0;
            for (int r = fil0; r <= fil1; r++)
            {
                if (!string.Equals(Texto(datos[r, colCodigo]), prefijo, StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }
                int cons;
                if (!int.TryParse(Texto(datos[r, colCons]), NumberStyles.Integer, CultureInfo.InvariantCulture, out cons))
                {
                    continue;
                }
                if (Texto(datos[r, colNombre]).Length == 0)
                {
                    detalle = "primera fila libre de " + Hoja(prefijo) + " (fila " + (r - fil0 + 1) + ")";
                    return cons;
                }
                maxConNombre = Math.Max(maxConNombre, cons);
            }
            detalle = "mayor consecutivo usado en " + Hoja(prefijo) + " + 1";
            return maxConNombre + 1;
        }

        private static string Texto(object v)
        {
            if (v == null)
            {
                return "";
            }
            if (v is double)
            {
                return ((double)v).ToString("0.############", CultureInfo.InvariantCulture);
            }
            return Convert.ToString(v, CultureInfo.InvariantCulture).Trim();
        }

        private static object[,] LeerHoja(string ruta, string hoja)
        {
            Type tipo = Type.GetTypeFromProgID("Excel.Application");
            if (tipo == null)
            {
                throw new InvalidOperationException("Excel no esta instalado en este PC.");
            }
            object app = null, libros = null, libro = null, hojas = null, h = null, rango = null;
            try
            {
                app = Activator.CreateInstance(tipo);
                Set(app, "Visible", false);
                Set(app, "DisplayAlerts", false);
                libros = Get(app, "Workbooks");
                // Open(Filename, UpdateLinks=0, ReadOnly=true)
                libro = Call(libros, "Open", ruta, 0, true);
                hojas = Get(libro, "Worksheets");
                h = hojas.GetType().InvokeMember("Item", BindingFlags.GetProperty, null, hojas, new object[] { hoja });
                rango = Get(h, "UsedRange");
                object valor = Get(rango, "Value2");
                object[,] datos = valor as object[,];
                if (datos == null)
                {
                    throw new InvalidOperationException("La hoja " + hoja + " esta vacia.");
                }
                return datos;
            }
            finally
            {
                if (libro != null)
                {
                    try { Call(libro, "Close", false); } catch { }
                }
                if (app != null)
                {
                    try { Call(app, "Quit"); } catch { }
                }
                foreach (object o in new[] { rango, h, hojas, libro, libros, app })
                {
                    if (o != null && Marshal.IsComObject(o))
                    {
                        Marshal.FinalReleaseComObject(o);
                    }
                }
            }
        }

        private static object Get(object o, string prop)
        {
            return o.GetType().InvokeMember(prop, BindingFlags.GetProperty, null, o, null);
        }

        private static void Set(object o, string prop, object valor)
        {
            o.GetType().InvokeMember(prop, BindingFlags.SetProperty, null, o, new[] { valor });
        }

        private static object Call(object o, string metodo, params object[] args)
        {
            return o.GetType().InvokeMember(metodo, BindingFlags.InvokeMethod, null, o, args);
        }
    }
}
