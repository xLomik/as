// Genera el archivo de configuracion del Part Importer (.pis) para un programa,
// a partir de una plantilla (un .pis guardado por el propio Part Importer).
// Solo cambia material, espesor, maquina, regla de corte, .PAR y carpeta destino;
// el resto (tolerancias, orden de procesamiento, rotacion...) queda como en la plantilla.

using System;
using System.Globalization;
using System.Text;
using System.Xml;
using System.Xml.Linq;

namespace AutoBySoft
{
    public static class Pis
    {
        public static string Generar(string plantillaXml, Guid material, double espesor, Guid maquina,
                                     Guid reglaDeCorte, string archivoPar, string rutaLocalDestino, bool sobrescribir)
        {
            XDocument doc = XDocument.Parse(plantillaXml);
            XElement raiz = doc.Root;
            if (raiz == null || raiz.Name.LocalName != "ImportSettings")
            {
                throw new InvalidOperationException("La plantilla .pis no es valida (falta <ImportSettings>).");
            }
            raiz.SetAttributeValue("MaterialGuid", material.ToString());
            raiz.SetAttributeValue("Thickness", espesor.ToString("0.###", CultureInfo.InvariantCulture));
            raiz.SetAttributeValue("CuttingMachineGuid", maquina.ToString());
            raiz.SetAttributeValue("CuttingRuleGuid", reglaDeCorte.ToString());
            raiz.SetAttributeValue("NcParameterFile", archivoPar);
            raiz.SetAttributeValue("SavePathRelative", rutaLocalDestino);
            // Sin gas fijo: el Part Importer toma el gas de la tabla de espesores del material.
            raiz.SetAttributeValue("CuttingGasTypeGuid", null);
            // Si la pieza ya existe en esa carpeta: se ignora (por defecto) o se sobrescribe
            // (modo actualizar). Nunca "Indexing", que crearia otro nombre.
            raiz.SetAttributeValue("HandleFileConflicts", sobrescribir ? "Overwrite" : null);

            XmlWriterSettings s = new XmlWriterSettings();
            s.Encoding = new UTF8Encoding(false);
            s.Indent = true;
            s.IndentChars = "\t";
            StringBuilder sb = new StringBuilder();
            using (XmlWriter w = XmlWriter.Create(new Utf8StringWriter(sb), s))
            {
                doc.Save(w);
            }
            return sb.ToString();
        }

        private sealed class Utf8StringWriter : System.IO.StringWriter
        {
            public Utf8StringWriter(StringBuilder sb) : base(sb, CultureInfo.InvariantCulture) { }

            public override Encoding Encoding
            {
                get { return new UTF8Encoding(false); }
            }
        }
    }
}
