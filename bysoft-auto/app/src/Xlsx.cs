// Lectura y escritura minima de .xlsx (Office Open XML) sin depender de Excel.
// - XlsxLector: lee la primera hoja (o una por nombre) como filas de texto.
// - XlsxEscritor: escribe una hoja con todas las celdas como TEXTO (inlineStr),
//   que es lo que exige el Part Nester (hace (string)Value2 en cada celda).

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Xml.Linq;

namespace AutoBySoft
{
    public static class XlsxLector
    {
        private static readonly XNamespace Ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        private static readonly XNamespace NsRel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        private static readonly XNamespace NsPkgRel = "http://schemas.openxmlformats.org/package/2006/relationships";

        // Devuelve las filas de la hoja indicada (o la primera si hoja == null).
        // Cada fila es un arreglo de textos; las celdas vacias son "".
        public static List<string[]> Leer(string ruta, string hoja)
        {
            using (FileStream fs = new FileStream(ruta, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
            using (ZipArchive zip = new ZipArchive(fs, ZipArchiveMode.Read))
            {
                List<string> compartidos = LeerCompartidos(zip);
                string rutaHoja = BuscarHoja(zip, hoja);
                XDocument doc = CargarXml(zip, rutaHoja);
                if (doc == null)
                {
                    throw new InvalidDataException("No se encontro la hoja '" + (hoja ?? "(primera)") + "' en " + Path.GetFileName(ruta));
                }
                List<string[]> filas = new List<string[]>();
                foreach (XElement row in doc.Descendants(Ns + "row"))
                {
                    int numFila = ParseEntero((string)row.Attribute("r"), filas.Count + 1);
                    while (filas.Count < numFila - 1)
                    {
                        filas.Add(new string[0]);
                    }
                    Dictionary<int, string> celdas = new Dictionary<int, string>();
                    int siguiente = 0;
                    foreach (XElement c in row.Elements(Ns + "c"))
                    {
                        string referencia = (string)c.Attribute("r");
                        int col = referencia != null ? ColumnaDeReferencia(referencia) : siguiente;
                        siguiente = col + 1;
                        celdas[col] = ValorCelda(c, compartidos);
                    }
                    int ancho = celdas.Count == 0 ? 0 : celdas.Keys.Max() + 1;
                    string[] fila = new string[ancho];
                    for (int i = 0; i < ancho; i++)
                    {
                        string v;
                        fila[i] = celdas.TryGetValue(i, out v) ? v : "";
                    }
                    filas.Add(fila);
                }
                return filas;
            }
        }

        private static string ValorCelda(XElement c, List<string> compartidos)
        {
            string tipo = (string)c.Attribute("t");
            if (tipo == "inlineStr")
            {
                XElement isEl = c.Element(Ns + "is");
                return isEl == null ? "" : string.Concat(isEl.Descendants(Ns + "t").Select(t => (string)t));
            }
            XElement vEl = c.Element(Ns + "v");
            string v = vEl == null ? "" : (string)vEl;
            if (tipo == "s")
            {
                int idx;
                if (int.TryParse(v, NumberStyles.Integer, CultureInfo.InvariantCulture, out idx) && idx >= 0 && idx < compartidos.Count)
                {
                    return compartidos[idx];
                }
                return "";
            }
            if (tipo == "b")
            {
                return v == "1" ? "VERDADERO" : "FALSO";
            }
            if (tipo == null || tipo == "n")
            {
                // Numero: normalizar (Excel guarda 14 como "14", 3.42 como "3.42" o "3.4199999999999999")
                double d;
                if (double.TryParse(v, NumberStyles.Float, CultureInfo.InvariantCulture, out d))
                {
                    return d.ToString("0.############", CultureInfo.InvariantCulture);
                }
            }
            return v;
        }

        private static List<string> LeerCompartidos(ZipArchive zip)
        {
            List<string> lista = new List<string>();
            XDocument doc = CargarXml(zip, "xl/sharedStrings.xml");
            if (doc == null)
            {
                return lista;
            }
            foreach (XElement si in doc.Root.Elements(Ns + "si"))
            {
                lista.Add(string.Concat(si.Descendants(Ns + "t").Select(t => (string)t)));
            }
            return lista;
        }

        private static string BuscarHoja(ZipArchive zip, string nombre)
        {
            XDocument wb = CargarXml(zip, "xl/workbook.xml");
            XDocument rels = CargarXml(zip, "xl/_rels/workbook.xml.rels");
            if (wb == null || rels == null)
            {
                return "xl/worksheets/sheet1.xml";
            }
            XElement hoja = null;
            foreach (XElement s in wb.Descendants(Ns + "sheet"))
            {
                if (string.IsNullOrEmpty(nombre) || string.Equals((string)s.Attribute("name"), nombre, StringComparison.OrdinalIgnoreCase))
                {
                    hoja = s;
                    break;
                }
            }
            if (hoja == null)
            {
                return null;
            }
            string id = (string)hoja.Attribute(NsRel + "id");
            foreach (XElement r in rels.Root.Elements(NsPkgRel + "Relationship"))
            {
                if ((string)r.Attribute("Id") == id)
                {
                    string target = ((string)r.Attribute("Target")).Replace('\\', '/');
                    if (target.StartsWith("/"))
                    {
                        return target.TrimStart('/');
                    }
                    return target.StartsWith("xl/") ? target : "xl/" + target;
                }
            }
            return null;
        }

        private static XDocument CargarXml(ZipArchive zip, string ruta)
        {
            if (ruta == null)
            {
                return null;
            }
            ZipArchiveEntry e = zip.Entries.FirstOrDefault(x => string.Equals(x.FullName, ruta, StringComparison.OrdinalIgnoreCase));
            if (e == null)
            {
                return null;
            }
            using (Stream s = e.Open())
            {
                return XDocument.Load(s);
            }
        }

        private static int ParseEntero(string s, int defecto)
        {
            int n;
            return int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out n) ? n : defecto;
        }

        // "B12" -> 1 (columna base 0)
        public static int ColumnaDeReferencia(string referencia)
        {
            int col = 0;
            foreach (char ch in referencia)
            {
                if (ch >= 'A' && ch <= 'Z')
                {
                    col = col * 26 + (ch - 'A' + 1);
                }
                else if (ch >= 'a' && ch <= 'z')
                {
                    col = col * 26 + (ch - 'a' + 1);
                }
                else
                {
                    break;
                }
            }
            return col - 1;
        }
    }

    public static class XlsxEscritor
    {
        // Escribe una sola hoja; todas las celdas como texto. Sin encabezado salvo que
        // el llamador lo incluya en 'filas'.
        public static void Escribir(string ruta, string nombreHoja, IList<string[]> filas)
        {
            string dir = Path.GetDirectoryName(ruta);
            if (!string.IsNullOrEmpty(dir))
            {
                Directory.CreateDirectory(dir);
            }
            if (File.Exists(ruta))
            {
                File.Delete(ruta);
            }
            using (FileStream fs = new FileStream(ruta, FileMode.CreateNew))
            using (ZipArchive zip = new ZipArchive(fs, ZipArchiveMode.Create))
            {
                Entrada(zip, "[Content_Types].xml",
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>" +
                    "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">" +
                    "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>" +
                    "<Default Extension=\"xml\" ContentType=\"application/xml\"/>" +
                    "<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>" +
                    "<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>" +
                    "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>" +
                    "</Types>");
                Entrada(zip, "_rels/.rels",
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>" +
                    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">" +
                    "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>" +
                    "</Relationships>");
                Entrada(zip, "xl/workbook.xml",
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>" +
                    "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">" +
                    "<sheets><sheet name=\"" + Esc(nombreHoja) + "\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>");
                Entrada(zip, "xl/_rels/workbook.xml.rels",
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>" +
                    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">" +
                    "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>" +
                    "<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>" +
                    "</Relationships>");
                // Estilo 1 = formato de texto (@), para que Excel muestre todo como texto.
                Entrada(zip, "xl/styles.xml",
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>" +
                    "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">" +
                    "<fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>" +
                    "<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>" +
                    "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>" +
                    "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>" +
                    "<cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>" +
                    "<xf numFmtId=\"49\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/></cellXfs>" +
                    "<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>" +
                    "</styleSheet>");

                StringBuilder sb = new StringBuilder();
                sb.Append("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>");
                sb.Append("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>");
                for (int r = 0; r < filas.Count; r++)
                {
                    sb.Append("<row r=\"").Append(r + 1).Append("\">");
                    string[] fila = filas[r] ?? new string[0];
                    for (int c = 0; c < fila.Length; c++)
                    {
                        string v = fila[c];
                        if (string.IsNullOrEmpty(v))
                        {
                            continue;   // celda vacia: BySoft la lee como null -> ""
                        }
                        sb.Append("<c r=\"").Append(NombreColumna(c)).Append(r + 1)
                          .Append("\" s=\"1\" t=\"inlineStr\"><is><t xml:space=\"preserve\">")
                          .Append(Esc(v)).Append("</t></is></c>");
                    }
                    sb.Append("</row>");
                }
                sb.Append("</sheetData></worksheet>");
                Entrada(zip, "xl/worksheets/sheet1.xml", sb.ToString());
            }
        }

        public static string NombreColumna(int c)
        {
            string s = "";
            c++;
            while (c > 0)
            {
                int m = (c - 1) % 26;
                s = (char)('A' + m) + s;
                c = (c - 1) / 26;
            }
            return s;
        }

        private static void Entrada(ZipArchive zip, string nombre, string contenido)
        {
            ZipArchiveEntry e = zip.CreateEntry(nombre, CompressionLevel.Optimal);
            using (StreamWriter w = new StreamWriter(e.Open(), new UTF8Encoding(false)))
            {
                w.Write(contenido);
            }
        }

        private static string Esc(string s)
        {
            StringBuilder sb = new StringBuilder(s.Length);
            foreach (char ch in s)
            {
                switch (ch)
                {
                    case '&': sb.Append("&amp;"); break;
                    case '<': sb.Append("&lt;"); break;
                    case '>': sb.Append("&gt;"); break;
                    case '"': sb.Append("&quot;"); break;
                    default:
                        // Caracteres de control no validos en XML 1.0
                        if (ch < 0x20 && ch != '\t' && ch != '\n' && ch != '\r')
                        {
                            break;
                        }
                        sb.Append(ch);
                        break;
                }
            }
            return sb.ToString();
        }
    }
}
