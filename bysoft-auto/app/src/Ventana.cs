// Ventana principal: el usuario elige la carpeta del pedido, el Excel de cantidades,
// el tipo de programa (LP/LD), la carpeta destino en BySoft y el consecutivo.
// "1. Revisar" valida todo sin modificar BySoft; "2. Ejecutar" hace el trabajo.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace AutoBySoft
{
    public sealed class Ventana : Form
    {
        private readonly Config _cfg;
        private readonly TextBox _txtPedido = new TextBox();
        private readonly TextBox _txtExcel = new TextBox();
        private readonly RadioButton _rbLP = new RadioButton();
        private readonly RadioButton _rbLD = new RadioButton();
        private readonly TextBox _txtDestino = new TextBox();
        private readonly TextBox _txtListado = new TextBox();
        private readonly NumericUpDown _numCons = new NumericUpDown();
        private readonly ComboBox _cmbExistentes = new ComboBox();
        private readonly Button _btnRevisar = new Button();
        private readonly Button _btnEjecutar = new Button();
        private readonly Button _btnAbrir = new Button();
        private readonly TextBox _log = new TextBox();
        private Plan _plan;
        private Entrada _entrada;
        private string _salida;

        public Ventana(Config cfg)
        {
            _cfg = cfg;
            Text = "AutoBySoft - Importar piezas y preparar nesteos";
            Font = new Font("Segoe UI", 9f);
            ClientSize = new Size(900, 700);
            MinimumSize = new Size(760, 560);
            StartPosition = FormStartPosition.CenterScreen;

            TableLayoutPanel t = new TableLayoutPanel();
            t.Dock = DockStyle.Fill;
            t.Padding = new Padding(10);
            t.ColumnCount = 3;
            t.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 210));
            t.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            t.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110));
            Controls.Add(t);

            Fila(t, "Carpeta del pedido (DXF):", _txtPedido, Boton("Buscar...", ElegirPedido));
            Fila(t, "Excel de cantidades:", _txtExcel, Boton("Buscar...", ElegirExcel));

            FlowLayoutPanel tipo = new FlowLayoutPanel();
            tipo.AutoSize = true;
            _rbLP.Text = "Produccion (LP)";
            _rbLP.AutoSize = true;
            _rbLD.Text = "Desarrollo (LD)";
            _rbLD.AutoSize = true;
            _rbLD.Checked = true;
            tipo.Controls.Add(_rbLP);
            tipo.Controls.Add(_rbLD);
            Fila(t, "Tipo de programa:", tipo, null);

            Fila(t, "Carpeta destino en BySoft:", _txtDestino, null);
            Label ayuda = new Label();
            ayuda.Text = "Ej.: DESARROLLO\\150. Ref 10510104995   (dentro se crea una subcarpeta por material/espesor)";
            ayuda.ForeColor = Color.DimGray;
            ayuda.AutoSize = true;
            t.Controls.Add(new Label(), 0, t.RowCount);
            t.Controls.Add(ayuda, 1, t.RowCount);
            t.RowCount++;

            Fila(t, "Listado de programas (.xls):", _txtListado, Boton("Buscar...", ElegirListado));
            FlowLayoutPanel cons = new FlowLayoutPanel();
            cons.AutoSize = true;
            _numCons.Minimum = 0;
            _numCons.Maximum = 999999;
            _numCons.Width = 90;
            cons.Controls.Add(_numCons);
            cons.Controls.Add(Boton("Leer del listado", LeerConsecutivo));
            Fila(t, "Consecutivo del 1er programa:", cons, null);

            _cmbExistentes.DropDownStyle = ComboBoxStyle.DropDownList;
            _cmbExistentes.Items.Add("Detener y avisar (no tocar nada)");
            _cmbExistentes.Items.Add("Usar la existente (no volver a importarla)");
            _cmbExistentes.Items.Add("Actualizar conservando nesteos anteriores (renombra la version vieja)");
            _cmbExistentes.Items.Add("Actualizar y sobrescribir (los nesteos anteriores tambien cambian)");
            _cmbExistentes.SelectedIndex = 0;
            Fila(t, "Si la pieza ya existe en BySoft:", _cmbExistentes, null);

            FlowLayoutPanel botones = new FlowLayoutPanel();
            botones.AutoSize = true;
            _btnRevisar.Text = "1. Revisar";
            _btnRevisar.Size = new Size(140, 34);
            _btnRevisar.Click += delegate { Revisar(); };
            _btnEjecutar.Text = "2. Ejecutar";
            _btnEjecutar.Size = new Size(140, 34);
            _btnEjecutar.Enabled = false;
            _btnEjecutar.Click += delegate { Ejecutar(); };
            _btnAbrir.Text = "Abrir resultados";
            _btnAbrir.Size = new Size(140, 34);
            _btnAbrir.Enabled = false;
            _btnAbrir.Click += delegate { if (_salida != null) System.Diagnostics.Process.Start("explorer.exe", "\"" + _salida + "\""); };
            botones.Controls.Add(_btnRevisar);
            botones.Controls.Add(_btnEjecutar);
            botones.Controls.Add(_btnAbrir);
            t.Controls.Add(botones, 0, t.RowCount);
            t.SetColumnSpan(botones, 3);
            t.RowCount++;

            _log.Multiline = true;
            _log.ScrollBars = ScrollBars.Both;
            _log.WordWrap = false;
            _log.ReadOnly = true;
            _log.Font = new Font("Consolas", 9f);
            _log.Dock = DockStyle.Fill;
            t.Controls.Add(_log, 0, t.RowCount);
            t.SetColumnSpan(_log, 3);
            t.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            t.RowCount++;

            foreach (Control c in new Control[] { _txtPedido, _txtExcel, _txtDestino, _numCons })
            {
                Control cc = c;
                cc.TextChanged += delegate { InvalidarRevision(); };
            }
            _rbLP.CheckedChanged += delegate { InvalidarRevision(); };
            _cmbExistentes.SelectedIndexChanged += delegate { InvalidarRevision(); };

            CargarPreferencias();
            FormClosing += delegate { GuardarPreferencias(); };
            Escribir("Paso 1: completa los datos y pulsa '1. Revisar'. No se modifica nada hasta pulsar '2. Ejecutar'.");
        }

        // ------------------------------------------------------------------ eventos
        private void ElegirPedido(object s, EventArgs e)
        {
            using (FolderBrowserDialog d = new FolderBrowserDialog())
            {
                d.Description = "Carpeta del pedido (con subcarpetas MATERIAL Esp=Xmm)";
                if (Directory.Exists(_txtPedido.Text)) d.SelectedPath = _txtPedido.Text;
                if (d.ShowDialog(this) == DialogResult.OK) _txtPedido.Text = d.SelectedPath;
            }
        }

        private void ElegirExcel(object s, EventArgs e)
        {
            using (OpenFileDialog d = new OpenFileDialog())
            {
                d.Filter = "Excel (*.xlsx)|*.xlsx";
                if (Directory.Exists(_txtPedido.Text)) d.InitialDirectory = _txtPedido.Text;
                if (d.ShowDialog(this) == DialogResult.OK) _txtExcel.Text = d.FileName;
            }
        }

        private void ElegirListado(object s, EventArgs e)
        {
            using (OpenFileDialog d = new OpenFileDialog())
            {
                d.Filter = "Listado de programas (*.xls;*.xlsx)|*.xls;*.xlsx";
                if (d.ShowDialog(this) == DialogResult.OK) _txtListado.Text = d.FileName;
            }
        }

        private void LeerConsecutivo(object s, EventArgs e)
        {
            if (!File.Exists(_txtListado.Text))
            {
                MessageBox.Show(this, "Elige primero el archivo del listado de programas.", Text);
                return;
            }
            Cursor = Cursors.WaitCursor;
            try
            {
                string detalle;
                int c = Listado.SiguienteConsecutivo(_txtListado.Text, Prefijo(), out detalle);
                _numCons.Value = c;
                Escribir("Consecutivo " + Prefijo() + " " + c + " (" + detalle + "). Verificalo antes de ejecutar.");
            }
            catch (Exception ex)
            {
                Escribir("No se pudo leer el listado: " + ex.Message + "  -> escribe el consecutivo a mano.");
            }
            finally
            {
                Cursor = Cursors.Default;
            }
        }

        private void Revisar()
        {
            _log.Clear();
            _entrada = LeerEntrada();
            Bloquear(true);
            EnSegundoPlano(delegate
            {
                Motor m = new Motor(_cfg, Escribir);
                Plan p = m.Revisar(_entrada);
                string resumen = m.Resumen(p);
                Invoke((MethodInvoker)delegate
                {
                    _plan = p;
                    Escribir("");
                    Escribir(resumen);
                    if (p.Ok)
                    {
                        Escribir("Revision correcta. Revisa el resumen y pulsa '2. Ejecutar'.");
                    }
                    else if (p.Errores.Count == 0)
                    {
                        Escribir("No hay nada que procesar.");
                    }
                    Bloquear(false);
                    _btnEjecutar.Enabled = p.Ok;
                });
            });
        }

        private void Ejecutar()
        {
            if (_plan == null || !_plan.Ok)
            {
                return;
            }
            if (MessageBox.Show(this, "Se crearan las carpetas en BySoft y se importaran las piezas de " + _plan.Programas.Count +
                                " programa(s). ¿Continuar?", Text, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            {
                return;
            }
            List<string> actualizadas = new List<string>();
            foreach (Programa pr in _plan.Programas)
            {
                foreach (Pieza pz in pr.Piezas)
                {
                    if (pz.Sobrescribir)
                    {
                        actualizadas.Add(pz.Referencia + "  (" + pz.CarpetaLocal + ")");
                    }
                }
            }
            if (actualizadas.Count > 0)
            {
                int max = Math.Min(actualizadas.Count, 15);
                string lista = string.Join("\n", actualizadas.GetRange(0, max).ToArray());
                if (actualizadas.Count > max)
                {
                    lista += "\n... y " + (actualizadas.Count - max) + " mas (ver registro).";
                }
                if (MessageBox.Show(this, "ATENCION: se van a SOBRESCRIBIR " + actualizadas.Count + " pieza(s) que ya existen en BySoft:\n\n" +
                                    lista + "\n\nLos nesteos ANTERIORES que usan estas piezas tambien veran el cambio " +
                                    "(estan enlazados a la pieza de la carpeta). ¿Sobrescribir de todas formas?",
                                    Text, MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2) != DialogResult.Yes)
                {
                    return;
                }
            }
            Bloquear(true);
            Plan plan = _plan;
            Entrada ent = _entrada;
            EnSegundoPlano(delegate
            {
                Motor m = new Motor(_cfg, Escribir);
                string salida;
                bool ok = m.Ejecutar(plan, ent, out salida);
                try
                {
                    File.WriteAllText(Path.Combine(salida, "registro.txt"), LeerLog(), new UTF8Encoding(true));
                }
                catch { }
                Invoke((MethodInvoker)delegate
                {
                    _salida = salida;
                    _btnAbrir.Enabled = true;
                    Bloquear(false);
                    _btnEjecutar.Enabled = false;   // obliga a revisar de nuevo
                    MessageBox.Show(this, ok ? "Terminado. Los Excel para el Part Nester estan en:\n" + salida
                                             : "Terminado CON ERRORES. Revisa el registro en:\n" + salida,
                                    Text, MessageBoxButtons.OK, ok ? MessageBoxIcon.Information : MessageBoxIcon.Warning);
                });
            });
        }

        // ------------------------------------------------------------------ utilidades
        private Entrada LeerEntrada()
        {
            Entrada e = new Entrada();
            e.CarpetaPedido = _txtPedido.Text.Trim().TrimEnd('\\');
            e.ExcelCantidades = _txtExcel.Text.Trim();
            e.Prefijo = Prefijo();
            e.DestinoBase = _txtDestino.Text.Trim().Trim('\\', '/');
            e.Consecutivo = (int)_numCons.Value;
            e.Existentes = (ModoExistentes)_cmbExistentes.SelectedIndex;
            return e;
        }

        private string Prefijo()
        {
            return _rbLP.Checked ? "LP" : "LD";
        }

        private void InvalidarRevision()
        {
            _plan = null;
            _btnEjecutar.Enabled = false;
        }

        private void Bloquear(bool ocupado)
        {
            _btnRevisar.Enabled = !ocupado;
            _btnEjecutar.Enabled = false;
            UseWaitCursor = ocupado;
        }

        private void EnSegundoPlano(ThreadStart trabajo)
        {
            Thread th = new Thread(delegate ()
            {
                try
                {
                    trabajo();
                }
                catch (Exception ex)
                {
                    Escribir("ERROR inesperado: " + ex);
                    try { Invoke((MethodInvoker)delegate { Bloquear(false); }); } catch { }
                }
            });
            th.IsBackground = true;
            th.SetApartmentState(ApartmentState.STA);
            th.Start();
        }

        private void Escribir(string linea)
        {
            if (InvokeRequired)
            {
                BeginInvoke((Action<string>)Escribir, linea);
                return;
            }
            _log.AppendText((linea ?? "").Replace("\n", "\r\n").Replace("\r\r\n", "\r\n") + "\r\n");
        }

        private string LeerLog()
        {
            if (InvokeRequired)
            {
                return (string)Invoke((Func<string>)LeerLog);
            }
            return _log.Text;
        }

        private static Button Boton(string texto, EventHandler h)
        {
            Button b = new Button();
            b.Text = texto;
            b.AutoSize = true;
            b.Click += h;
            return b;
        }

        private static void Fila(TableLayoutPanel t, string etiqueta, Control c, Control extra)
        {
            Label l = new Label();
            l.Text = etiqueta;
            l.AutoSize = true;
            l.Anchor = AnchorStyles.Left;
            l.Margin = new Padding(3, 8, 3, 3);
            t.Controls.Add(l, 0, t.RowCount);
            c.Dock = DockStyle.Fill;
            t.Controls.Add(c, 1, t.RowCount);
            if (extra != null)
            {
                t.Controls.Add(extra, 2, t.RowCount);
            }
            t.RowCount++;
        }

        // Preferencias del usuario: %APPDATA%\AutoBySoft\preferencias.txt
        private static string RutaPreferencias()
        {
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "AutoBySoft", "preferencias.txt");
        }

        private void CargarPreferencias()
        {
            _txtListado.Text = _cfg.Listado;
            try
            {
                string r = RutaPreferencias();
                if (!File.Exists(r)) return;
                foreach (string l in File.ReadAllLines(r, Encoding.UTF8))
                {
                    int i = l.IndexOf('=');
                    if (i <= 0) continue;
                    string k = l.Substring(0, i), v = l.Substring(i + 1);
                    if (k == "listado" && v.Length > 0) _txtListado.Text = v;
                    else if (k == "tipo") { _rbLP.Checked = v == "LP"; _rbLD.Checked = v != "LP"; }
                    else if (k == "existentes") { int i2; if (int.TryParse(v, out i2) && i2 >= 0 && i2 < _cmbExistentes.Items.Count) _cmbExistentes.SelectedIndex = i2; }
                }
            }
            catch { }
        }

        private void GuardarPreferencias()
        {
            try
            {
                string r = RutaPreferencias();
                Directory.CreateDirectory(Path.GetDirectoryName(r));
                File.WriteAllText(r, "listado=" + _txtListado.Text + "\r\ntipo=" + Prefijo() + "\r\nexistentes=" + _cmbExistentes.SelectedIndex + "\r\n", Encoding.UTF8);
            }
            catch { }
        }
    }
}
