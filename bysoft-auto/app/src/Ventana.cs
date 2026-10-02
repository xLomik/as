// Ventana principal, organizada en 3 pasos:
//   1. Pedido: carpeta con los DXF y Excel de cantidades (se puede generar desde los DXF).
//   2. Programa en BySoft: LP/LD, carpeta destino (con selector del arbol de BySoft),
//      listado y consecutivo (con vista previa de los nombres).
//   3. Que hacer si una pieza ya existe.
// "Revisar" valida todo sin modificar BySoft; "Ejecutar" hace el trabajo.
// A la derecha: resumen por programa, problemas (errores/avisos) y registro detallado.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace AutoBySoft
{
    public sealed class Ventana : Form
    {
        private static readonly Color Azul = Color.FromArgb(0, 84, 147);
        private static readonly Color Verde = Color.FromArgb(16, 124, 16);
        private static readonly Color Gris = Color.FromArgb(96, 96, 96);
        private static readonly Color Rojo = Color.FromArgb(196, 43, 28);
        private static readonly Color Naranja = Color.FromArgb(157, 93, 0);

        private readonly Config _cfg;
        private readonly TextBox _txtPedido = new TextBox();
        private readonly Label _infoPedido = new Label();
        private readonly TextBox _txtExcel = new TextBox();
        private readonly Label _infoExcel = new Label();
        private readonly RadioButton _rbLP = new RadioButton();
        private readonly RadioButton _rbLD = new RadioButton();
        private readonly TextBox _txtDestino = new TextBox();
        private readonly TextBox _txtListado = new TextBox();
        private readonly NumericUpDown _numCons = new NumericUpDown();
        private readonly Label _infoNombres = new Label();
        private readonly RadioButton[] _rbExistentes = new RadioButton[4];
        private readonly Button _btnRevisar = new Button();
        private readonly Button _btnEjecutar = new Button();
        private readonly Button _btnAbrir = new Button();
        private readonly Label _estado = new Label();
        private readonly ProgressBar _progreso = new ProgressBar();
        private readonly TabControl _tabs = new TabControl();
        private readonly TabPage _tabResumen = new TabPage("Resumen");
        private readonly TabPage _tabProblemas = new TabPage("Problemas");
        private readonly TabPage _tabRegistro = new TabPage("Registro detallado");
        private readonly ListView _lvResumen = new ListView();
        private readonly ListView _lvProblemas = new ListView();
        private readonly TextBox _log = new TextBox();
        private readonly ToolTip _tips = new ToolTip();
        private BySoftApi _api;
        private int _subcarpetasConDxf;
        private Plan _plan;
        private Entrada _entrada;
        private string _salida;

        public Ventana(Config cfg)
        {
            _cfg = cfg;
            Text = "AutoBySoft - Importar piezas y preparar nesteos";
            Font = new Font("Segoe UI", 9f);
            ClientSize = new Size(1260, 780);
            MinimumSize = new Size(980, 640);
            StartPosition = FormStartPosition.CenterScreen;
            AllowDrop = true;
            DragEnter += AlArrastrar;
            DragDrop += AlSoltar;

            // ---- encabezado
            Panel cab = new Panel();
            cab.Dock = DockStyle.Top;
            cab.Height = 58;
            cab.BackColor = Azul;
            Label titulo = new Label();
            titulo.Text = "AutoBySoft";
            titulo.ForeColor = Color.White;
            titulo.Font = new Font("Segoe UI Semibold", 15f);
            titulo.AutoSize = true;
            titulo.Location = new Point(14, 4);
            Label sub = new Label();
            sub.Text = "Importa los DXF de un pedido a BySoft y prepara los Excel para el Part Nester. " +
                       "Puedes arrastrar la carpeta o los Excel a esta ventana.";
            sub.ForeColor = Color.FromArgb(220, 232, 245);
            sub.AutoSize = true;
            sub.Location = new Point(16, 34);
            cab.Controls.Add(titulo);
            cab.Controls.Add(sub);

            // ---- columna izquierda: pasos
            Panel izq = new Panel();
            izq.Dock = DockStyle.Left;
            izq.Width = 545;
            izq.AutoScroll = true;
            izq.Padding = new Padding(10, 8, 6, 8);

            FlowLayoutPanel pasos = new FlowLayoutPanel();
            pasos.FlowDirection = FlowDirection.TopDown;
            pasos.WrapContents = false;
            pasos.AutoSize = true;
            pasos.Dock = DockStyle.Top;
            izq.Controls.Add(pasos);

            pasos.Controls.Add(CrearPaso1());
            pasos.Controls.Add(CrearPaso2());
            pasos.Controls.Add(CrearPaso3());
            pasos.Controls.Add(CrearAcciones());

            // ---- derecha: resultados
            _tabs.Dock = DockStyle.Fill;
            _tabs.Padding = new Point(12, 4);
            ConfigurarLista(_lvResumen, new[] { "Programa", "Subcarpeta", "Material", "Esp.", ".PAR", "Piezas", "Nuevas", "Ya existen" },
                            new[] { 85, 160, 60, 40, 210, 50, 55, 70 });
            ConfigurarLista(_lvProblemas, new[] { "Tipo", "Detalle" }, new[] { 70, 900 });
            _tabResumen.Controls.Add(_lvResumen);
            _tabProblemas.Controls.Add(_lvProblemas);
            _log.Multiline = true;
            _log.ScrollBars = ScrollBars.Both;
            _log.WordWrap = false;
            _log.ReadOnly = true;
            _log.BackColor = Color.White;
            _log.Font = new Font("Consolas", 9f);
            _log.Dock = DockStyle.Fill;
            _tabRegistro.Controls.Add(_log);
            _tabs.TabPages.Add(_tabResumen);
            _tabs.TabPages.Add(_tabProblemas);
            _tabs.TabPages.Add(_tabRegistro);
            Panel der = new Panel();
            der.Dock = DockStyle.Fill;
            der.Padding = new Padding(4, 10, 10, 10);
            der.Controls.Add(_tabs);

            Controls.Add(der);
            Controls.Add(izq);
            Controls.Add(cab);

            // ---- eventos de cambio
            _txtPedido.TextChanged += delegate { InvalidarRevision(); AnalizarPedido(); };
            _txtExcel.TextChanged += delegate { InvalidarRevision(); AnalizarExcel(); };
            _txtDestino.TextChanged += delegate { InvalidarRevision(); };
            _numCons.ValueChanged += delegate { InvalidarRevision(); ActualizarNombres(); };
            _rbLP.CheckedChanged += delegate { InvalidarRevision(); ActualizarNombres(); };
            foreach (RadioButton rb in _rbExistentes)
            {
                rb.CheckedChanged += delegate { InvalidarRevision(); };
            }

            CargarPreferencias();
            FormClosing += delegate { GuardarPreferencias(); };
            AnalizarPedido();
            AnalizarExcel();
            ActualizarNombres();
            Estado("Completa los pasos 1 a 3 y pulsa Revisar. No se modifica nada en BySoft hasta pulsar Ejecutar.", Gris);
        }

        // ================================================================== construccion
        private Control CrearPaso1()
        {
            TableLayoutPanel t = Grupo("1  Pedido");
            Fila(t, "Carpeta del pedido:", _txtPedido, Boton("Buscar...", ElegirPedido));
            Info(t, _infoPedido);
            Fila(t, "Excel de cantidades:", _txtExcel, Boton("Buscar...", ElegirExcel));
            Info(t, _infoExcel);
            Button crear = Boton("Crear Excel de cantidades con los DXF del pedido", CrearExcelCantidades);
            _tips.SetToolTip(crear, "Crea CANTIDADES_<pedido>.xlsx con todas las referencias de los DXF.\n" +
                                    "Solo tienes que escribir las cantidades. Las filas sin cantidad no se importan.");
            crear.Margin = new Padding(3, 0, 3, 3);
            t.Controls.Add(crear, 1, t.RowCount);
            t.SetColumnSpan(crear, 2);
            t.RowCount++;
            _tips.SetToolTip(_txtPedido, "Carpeta con una subcarpeta por material y espesor, ej. 'SAEJ 050 Esp=3.42mm'.");
            return t.Parent;
        }

        private Control CrearPaso2()
        {
            TableLayoutPanel t = Grupo("2  Programa en BySoft");
            FlowLayoutPanel tipo = new FlowLayoutPanel();
            tipo.AutoSize = true;
            _rbLP.Text = "Produccion (LP)";
            _rbLP.AutoSize = true;
            _rbLD.Text = "Desarrollo (LD)";
            _rbLD.AutoSize = true;
            _rbLD.Checked = true;
            tipo.Controls.Add(_rbLP);
            tipo.Controls.Add(_rbLD);
            Fila(t, "Tipo:", tipo, null);

            Fila(t, "Carpeta destino:", _txtDestino, Boton("Elegir...", ElegirDestino));
            Label ayuda = new Label();
            ayuda.Text = "Ej. DESARROLLO\\150. Ref 10510104995. Dentro se crea una carpeta por material/espesor.";
            Info(t, ayuda);

            Fila(t, "Listado de programas:", _txtListado, Boton("Buscar...", ElegirListado));
            FlowLayoutPanel cons = new FlowLayoutPanel();
            cons.AutoSize = true;
            _numCons.Minimum = 0;
            _numCons.Maximum = 999999;
            _numCons.Width = 90;
            _numCons.Margin = new Padding(0, 4, 3, 3);
            cons.Controls.Add(_numCons);
            cons.Controls.Add(Boton("Leer del listado", LeerConsecutivo));
            Fila(t, "Consecutivo:", cons, null);
            _infoNombres.ForeColor = Azul;
            Info(t, _infoNombres);
            return t.Parent;
        }

        private Control CrearPaso3()
        {
            TableLayoutPanel t = Grupo("3  Si una pieza ya existe en BySoft");
            string[] titulos =
            {
                "Detener y avisar",
                "Usar la que ya esta",
                "Actualizar conservando los nesteos anteriores (recomendado)",
                "Actualizar y sobrescribir"
            };
            string[] detalles =
            {
                "No se toca nada; te dice que piezas ya existen.",
                "No se vuelve a importar; el nesteo usa la pieza que ya esta en BySoft.",
                "La pieza vieja pasa a llamarse REF_ANT_aammdd y los nesteos viejos la siguen usando. El DXF nuevo entra con su nombre.",
                "Reemplaza la pieza. Los nesteos anteriores que la usan TAMBIEN cambian."
            };
            for (int i = 0; i < 4; i++)
            {
                RadioButton rb = new RadioButton();
                rb.Text = titulos[i];
                rb.AutoSize = true;
                rb.Margin = new Padding(3, i == 0 ? 2 : 6, 3, 0);
                if (i == 2) rb.Font = new Font(Font, FontStyle.Bold);
                if (i == 3) rb.ForeColor = Rojo;
                _rbExistentes[i] = rb;
                t.Controls.Add(rb, 0, t.RowCount);
                t.SetColumnSpan(rb, 3);
                t.RowCount++;
                Label d = new Label();
                d.Text = detalles[i];
                d.ForeColor = Gris;
                d.MaximumSize = new Size(470, 0);
                d.AutoSize = true;
                d.Margin = new Padding(20, 0, 3, 0);
                t.Controls.Add(d, 0, t.RowCount);
                t.SetColumnSpan(d, 3);
                t.RowCount++;
            }
            _rbExistentes[0].Checked = true;
            return t.Parent;
        }

        private Control CrearAcciones()
        {
            FlowLayoutPanel t = new FlowLayoutPanel();
            t.FlowDirection = FlowDirection.TopDown;
            t.WrapContents = false;
            t.AutoSize = true;
            t.Margin = new Padding(0, 4, 0, 0);
            FlowLayoutPanel botones = new FlowLayoutPanel();
            botones.AutoSize = true;
            botones.Margin = new Padding(0);
            Estilo(_btnRevisar, "Revisar", Azul);
            _btnRevisar.Click += delegate { Revisar(); };
            Estilo(_btnEjecutar, "Ejecutar", Verde);
            _btnEjecutar.Enabled = false;
            _btnEjecutar.Click += delegate { Ejecutar(); };
            _btnAbrir.Text = "Abrir resultados";
            _btnAbrir.Size = new Size(140, 38);
            _btnAbrir.Enabled = false;
            _btnAbrir.Click += delegate { if (_salida != null) System.Diagnostics.Process.Start("explorer.exe", "\"" + _salida + "\""); };
            _tips.SetToolTip(_btnRevisar, "Comprueba todo sin modificar BySoft.");
            _tips.SetToolTip(_btnEjecutar, "Crea las carpetas, importa las piezas y genera los Excel para el Part Nester.");
            botones.Controls.Add(_btnRevisar);
            botones.Controls.Add(_btnEjecutar);
            botones.Controls.Add(_btnAbrir);
            t.Controls.Add(botones);
            _progreso.Style = ProgressBarStyle.Marquee;
            _progreso.MarqueeAnimationSpeed = 30;
            _progreso.Width = 500;
            _progreso.Height = 10;
            _progreso.Visible = false;
            t.Controls.Add(_progreso);
            _estado.AutoSize = true;
            _estado.MaximumSize = new Size(500, 0);
            _estado.Margin = new Padding(3, 6, 3, 3);
            t.Controls.Add(_estado);
            return t;
        }

        // ================================================================== paso 1
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

        // Cuenta subcarpetas y DXF; si el Excel esta vacio busca uno con encabezado "Referencia".
        private void AnalizarPedido()
        {
            _subcarpetasConDxf = 0;
            string dir = _txtPedido.Text.Trim();
            if (dir.Length == 0)
            {
                Marcar(_infoPedido, "Elige la carpeta que tiene una subcarpeta por material/espesor.", Gris);
                ActualizarNombres();
                return;
            }
            if (!Directory.Exists(dir))
            {
                Marcar(_infoPedido, "La carpeta no existe.", Rojo);
                ActualizarNombres();
                return;
            }
            int dxf = 0;
            List<string> sinEspesor = new List<string>();
            try
            {
                foreach (string sub in Directory.GetDirectories(dir))
                {
                    string nombre = Path.GetFileName(sub);
                    if (nombre.StartsWith("_AutoBySoft_", StringComparison.OrdinalIgnoreCase)) continue;
                    int n = ContarDxf(sub);
                    if (n == 0) continue;
                    _subcarpetasConDxf++;
                    dxf += n;
                    string m;
                    double esp;
                    if (!Reglas.InterpretarSubcarpeta(nombre, out m, out esp)) sinEspesor.Add(nombre);
                }
            }
            catch (Exception ex)
            {
                Marcar(_infoPedido, "No se pudo leer la carpeta: " + ex.Message, Rojo);
                return;
            }
            if (_subcarpetasConDxf == 0)
            {
                Marcar(_infoPedido, "No hay subcarpetas con DXF.", Rojo);
            }
            else if (sinEspesor.Count > 0)
            {
                Marcar(_infoPedido, _subcarpetasConDxf + " subcarpeta(s), " + dxf + " DXF. Sin espesor en el nombre: " +
                                    string.Join(", ", sinEspesor.ToArray()) + "  (usa 'MATERIAL Esp=4.5mm')", Naranja);
            }
            else
            {
                Marcar(_infoPedido, "✔ " + _subcarpetasConDxf + " subcarpeta(s) de material/espesor, " + dxf + " DXF.", Verde);
            }
            if (_txtExcel.Text.Trim().Length == 0)
            {
                string encontrado = BuscarExcelCantidades(dir);
                if (encontrado != null) _txtExcel.Text = encontrado;
            }
            ActualizarNombres();
        }

        private static int ContarDxf(string dir)
        {
            return Directory.GetFiles(dir).Count(f => string.Equals(Path.GetExtension(f), ".dxf", StringComparison.OrdinalIgnoreCase));
        }

        private static string BuscarExcelCantidades(string dir)
        {
            foreach (string f in Directory.GetFiles(dir, "*.xlsx"))
            {
                if (Path.GetFileName(f).StartsWith("~$")) continue;
                try
                {
                    List<string[]> filas = XlsxLector.Leer(f, "");
                    for (int i = 0; i < filas.Count && i < 20; i++)
                    {
                        if (filas[i].Length > 0 && string.Equals(filas[i][0].Trim(), "Referencia", StringComparison.OrdinalIgnoreCase))
                        {
                            return f;
                        }
                    }
                }
                catch { }
            }
            return null;
        }

        private void AnalizarExcel()
        {
            string f = _txtExcel.Text.Trim();
            if (f.Length == 0)
            {
                Marcar(_infoExcel, "Elige el Excel o crealo con el boton de abajo.", Gris);
                return;
            }
            if (!File.Exists(f))
            {
                Marcar(_infoExcel, "El archivo no existe.", Rojo);
                return;
            }
            try
            {
                List<string> err = new List<string>(), av = new List<string>();
                List<Pieza> piezas = Reglas.LeerCantidades(XlsxLector.Leer(f, ""), err, av);
                if (err.Count > 0)
                {
                    Marcar(_infoExcel, err[0] + (err.Count > 1 ? "  (+" + (err.Count - 1) + " mas)" : ""), Rojo);
                }
                else
                {
                    Marcar(_infoExcel, "✔ " + piezas.Count + " referencia(s) con cantidad, " + piezas.Sum(p => p.Cantidad) + " piezas en total.", Verde);
                }
            }
            catch (IOException)
            {
                Marcar(_infoExcel, "No se puede leer: ¿esta abierto en Excel? Guardalo, cierralo y pulsa Revisar.", Naranja);
            }
            catch (Exception ex)
            {
                Marcar(_infoExcel, "No se pudo leer: " + ex.Message, Rojo);
            }
        }

        // Crea un Excel con todas las referencias DXF del pedido; el usuario solo escribe cantidades.
        private void CrearExcelCantidades(object s, EventArgs e)
        {
            string dir = _txtPedido.Text.Trim().TrimEnd('\\');
            if (!Directory.Exists(dir))
            {
                MessageBox.Show(this, "Primero elige la carpeta del pedido.", Text);
                return;
            }
            string ruta = Path.Combine(dir, "CANTIDADES_" + Seguro(Path.GetFileName(dir)) + ".xlsx");
            if (File.Exists(ruta))
            {
                DialogResult r = MessageBox.Show(this, "Ya existe:\n" + ruta + "\n\nSi = abrirlo\nNo = reemplazarlo por uno nuevo sin cantidades",
                                                 Text, MessageBoxButtons.YesNoCancel, MessageBoxIcon.Question);
                if (r == DialogResult.Cancel) return;
                if (r == DialogResult.Yes)
                {
                    _txtExcel.Text = ruta;
                    Abrir(ruta);
                    return;
                }
            }
            List<string[]> filas = new List<string[]>();
            filas.Add(new[] { "Referencia", "Cantidad", "Observacion" });
            HashSet<string> vistos = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            try
            {
                foreach (string sub in Directory.GetDirectories(dir).OrderBy(x => x, StringComparer.OrdinalIgnoreCase))
                {
                    if (Path.GetFileName(sub).StartsWith("_AutoBySoft_", StringComparison.OrdinalIgnoreCase)) continue;
                    foreach (string f in Directory.GetFiles(sub).OrderBy(x => x, StringComparer.OrdinalIgnoreCase))
                    {
                        if (!string.Equals(Path.GetExtension(f), ".dxf", StringComparison.OrdinalIgnoreCase)) continue;
                        string r = Path.GetFileNameWithoutExtension(f);
                        if (vistos.Add(r)) filas.Add(new[] { r, "", "" });
                    }
                }
                if (filas.Count == 1)
                {
                    MessageBox.Show(this, "No se encontraron DXF en las subcarpetas del pedido.", Text);
                    return;
                }
                XlsxEscritor.Escribir(ruta, "Cantidades", filas);
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "No se pudo crear el Excel:\n" + ex.Message, Text, MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            _txtExcel.Text = ruta;
            Abrir(ruta);
            MessageBox.Show(this, "Se creo el Excel con " + (filas.Count - 1) + " referencias.\n\n" +
                                  "Escribe la cantidad en la columna B, guarda y cierra Excel.\n" +
                                  "Las referencias sin cantidad no se importan.", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            AnalizarExcel();
        }

        // ================================================================== paso 2
        private void ElegirDestino(object s, EventArgs e)
        {
            Cursor = Cursors.WaitCursor;
            try
            {
                if (_api == null) _api = new BySoftApi(_cfg.DirBySoft);
            }
            catch (Exception ex)
            {
                Cursor = Cursors.Default;
                MessageBox.Show(this, "No se pudo conectar con la base de BySoft:\n" + ex.Message + "\n\nEscribe la ruta a mano.", Text);
                return;
            }
            Cursor = Cursors.Default;
            using (SelectorCarpeta d = new SelectorCarpeta(_api, _txtDestino.Text.Trim()))
            {
                if (d.ShowDialog(this) == DialogResult.OK) _txtDestino.Text = d.Ruta;
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
                Estado("Consecutivo " + Prefijo() + " " + c + " (" + detalle + "). Verificalo antes de ejecutar.", Gris);
                Escribir("Consecutivo " + Prefijo() + " " + c + " (" + detalle + ").");
            }
            catch (Exception ex)
            {
                Estado("No se pudo leer el listado: " + ex.Message + ". Escribe el consecutivo a mano.", Rojo);
            }
            finally
            {
                Cursor = Cursors.Default;
            }
        }

        private void ActualizarNombres()
        {
            if (_numCons.Value <= 0)
            {
                Marcar(_infoNombres, "Escribe el consecutivo o pulsa 'Leer del listado'.", Gris);
                return;
            }
            int n = Math.Max(1, _subcarpetasConDxf);
            int c = (int)_numCons.Value;
            string primero = Reglas.NombrePrograma(Prefijo(), DateTime.Now, c);
            Marcar(_infoNombres, n == 1
                ? "Nombre del programa: " + primero
                : "Nombres: " + primero + " ... " + Reglas.NombrePrograma(Prefijo(), DateTime.Now, c + n - 1) +
                  "  (" + n + " programas, uno por subcarpeta)", Azul);
        }

        // ================================================================== revisar / ejecutar
        private void Revisar()
        {
            _log.Clear();
            _lvResumen.Items.Clear();
            _lvProblemas.Items.Clear();
            AnalizarExcel();
            _entrada = LeerEntrada();
            Bloquear(true, "Revisando... (no se modifica nada)");
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
                    MostrarPlan(p);
                    Bloquear(false, null);
                    _btnEjecutar.Enabled = p.Ok;
                    if (p.Ok)
                    {
                        Estado("✔ Revision correcta: " + p.Programas.Count + " programa(s). Revisa el resumen y pulsa Ejecutar." +
                               (p.Avisos.Count > 0 ? "  (" + p.Avisos.Count + " aviso(s) en Problemas)" : ""), Verde);
                        _tabs.SelectedTab = _tabResumen;
                    }
                    else if (p.Errores.Count == 0)
                    {
                        Estado("No hay nada que procesar.", Naranja);
                    }
                    else
                    {
                        Estado("✖ Hay " + p.Errores.Count + " error(es). Corrigelos (pestaña Problemas) y vuelve a Revisar.", Rojo);
                        _tabs.SelectedTab = _tabProblemas;
                    }
                });
            });
        }

        private void MostrarPlan(Plan p)
        {
            _lvResumen.BeginUpdate();
            _lvResumen.Items.Clear();
            foreach (Programa pr in p.Programas)
            {
                int existen = pr.Piezas.Count(x => x.UbicacionesEnBySoft.Count > 0);
                ListViewItem it = new ListViewItem(new[]
                {
                    pr.Nombre ?? "", pr.Subcarpeta ?? "",
                    pr.MaterialBySoft ?? "", Reglas.Num(pr.EspesorReal),
                    pr.Par == null ? "" : pr.Par.Archivo,
                    pr.Piezas.Count.ToString(), (pr.Piezas.Count - existen).ToString(), existen.ToString()
                });
                it.ToolTipText = "Destino: " + pr.DestinoRelativo;
                _lvResumen.Items.Add(it);
            }
            _lvResumen.EndUpdate();

            _lvProblemas.BeginUpdate();
            _lvProblemas.Items.Clear();
            foreach (string er in p.Errores)
            {
                ListViewItem it = new ListViewItem(new[] { "ERROR", er });
                it.ForeColor = Rojo;
                it.ToolTipText = er;
                _lvProblemas.Items.Add(it);
            }
            foreach (string av in p.Avisos)
            {
                ListViewItem it = new ListViewItem(new[] { "Aviso", av });
                it.ForeColor = Naranja;
                it.ToolTipText = av;
                _lvProblemas.Items.Add(it);
            }
            _lvProblemas.EndUpdate();
            _tabProblemas.Text = "Problemas (" + p.Errores.Count + " errores, " + p.Avisos.Count + " avisos)";
        }

        private void Ejecutar()
        {
            if (_plan == null || !_plan.Ok)
            {
                return;
            }
            int piezas = _plan.Programas.Sum(x => x.Piezas.Count(z => z.Importar));
            if (MessageBox.Show(this, "Se crearan las carpetas en BySoft y se importaran " + piezas + " pieza(s) de " +
                                _plan.Programas.Count + " programa(s). ¿Continuar?", Text,
                                MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
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
            Bloquear(true, "Ejecutando... no cierres la ventana ni abras estas piezas en BySoft.");
            _tabs.SelectedTab = _tabRegistro;
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
                    Bloquear(false, null);
                    _btnEjecutar.Enabled = false;   // obliga a revisar de nuevo
                    if (ok)
                    {
                        Estado("✔ Terminado. Abre los resultados y carga cada Excel en el Part Nester " +
                               "(Importar piezas desde archivo).", Verde);
                    }
                    else
                    {
                        Estado("✖ Terminado CON ERRORES. Revisa el registro detallado.", Rojo);
                    }
                    if (MessageBox.Show(this, (ok ? "Terminado. Los Excel para el Part Nester estan en:\n"
                                                  : "Terminado CON ERRORES. Revisa el registro en:\n") + salida + "\n\n¿Abrir la carpeta?",
                                        Text, MessageBoxButtons.YesNo, ok ? MessageBoxIcon.Information : MessageBoxIcon.Warning) == DialogResult.Yes)
                    {
                        System.Diagnostics.Process.Start("explorer.exe", "\"" + salida + "\"");
                    }
                });
            });
        }

        // ================================================================== arrastrar y soltar
        private void AlArrastrar(object s, DragEventArgs e)
        {
            e.Effect = e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None;
        }

        // Carpeta -> pedido; .xls o .xlsx con "LISTADO" en el nombre -> listado; otro .xlsx -> cantidades.
        private void AlSoltar(object s, DragEventArgs e)
        {
            string[] rutas = e.Data.GetData(DataFormats.FileDrop) as string[];
            if (rutas == null) return;
            foreach (string r in rutas)
            {
                string ext = Path.GetExtension(r).ToLowerInvariant();
                if (Directory.Exists(r)) _txtPedido.Text = r;
                else if (ext == ".xls") _txtListado.Text = r;
                else if (ext == ".xlsx")
                {
                    if (Path.GetFileName(r).IndexOf("LISTADO", StringComparison.OrdinalIgnoreCase) >= 0) _txtListado.Text = r;
                    else _txtExcel.Text = r;
                }
            }
        }

        // ================================================================== utilidades
        private Entrada LeerEntrada()
        {
            Entrada e = new Entrada();
            e.CarpetaPedido = _txtPedido.Text.Trim().TrimEnd('\\');
            e.ExcelCantidades = _txtExcel.Text.Trim();
            e.Prefijo = Prefijo();
            e.DestinoBase = _txtDestino.Text.Trim().Trim('\\', '/');
            e.Consecutivo = (int)_numCons.Value;
            e.Existentes = (ModoExistentes)IndiceExistentes();
            return e;
        }

        private int IndiceExistentes()
        {
            for (int i = 0; i < _rbExistentes.Length; i++)
            {
                if (_rbExistentes[i].Checked) return i;
            }
            return 0;
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

        private void Bloquear(bool ocupado, string mensaje)
        {
            _btnRevisar.Enabled = !ocupado;
            _btnEjecutar.Enabled = false;
            _progreso.Visible = ocupado;
            UseWaitCursor = ocupado;
            if (mensaje != null) Estado(mensaje, Azul);
        }

        private void Estado(string texto, Color color)
        {
            _estado.Text = texto;
            _estado.ForeColor = color;
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
                    try
                    {
                        Invoke((MethodInvoker)delegate
                        {
                            Bloquear(false, null);
                            Estado("✖ Error inesperado: " + ex.Message, Rojo);
                            _tabs.SelectedTab = _tabRegistro;
                        });
                    }
                    catch { }
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

        private static void Abrir(string ruta)
        {
            try { System.Diagnostics.Process.Start(ruta); } catch { }
        }

        private static string Seguro(string s)
        {
            foreach (char c in Path.GetInvalidFileNameChars()) s = s.Replace(c, '_');
            return s;
        }

        private static void Marcar(Label l, string texto, Color color)
        {
            l.Text = texto;
            l.ForeColor = color;
        }

        private static Button Boton(string texto, EventHandler h)
        {
            Button b = new Button();
            b.Text = texto;
            b.AutoSize = true;
            b.Click += h;
            return b;
        }

        private static void Estilo(Button b, string texto, Color color)
        {
            b.Text = texto;
            b.Size = new Size(140, 38);
            b.FlatStyle = FlatStyle.Flat;
            b.FlatAppearance.BorderSize = 0;
            b.BackColor = color;
            b.ForeColor = Color.White;
            b.Font = new Font("Segoe UI Semibold", 10f);
            b.EnabledChanged += delegate
            {
                b.BackColor = b.Enabled ? color : Color.FromArgb(200, 200, 200);
            };
        }

        // GroupBox de ancho fijo con un TableLayoutPanel de 3 columnas (etiqueta | control | boton).
        // Devuelve la tabla; el GroupBox es t.Parent.
        private TableLayoutPanel Grupo(string titulo)
        {
            GroupBox g = new GroupBox();
            g.Text = titulo;
            g.Font = new Font("Segoe UI Semibold", 10f);
            g.ForeColor = Azul;
            g.AutoSize = true;
            g.AutoSizeMode = AutoSizeMode.GrowAndShrink;
            g.MinimumSize = new Size(512, 0);
            g.MaximumSize = new Size(512, 0);
            g.Padding = new Padding(8, 6, 8, 8);
            g.Margin = new Padding(0, 0, 0, 10);
            TableLayoutPanel t = new TableLayoutPanel();
            t.Font = Font;
            t.ForeColor = SystemColors.ControlText;
            t.Dock = DockStyle.Top;
            t.AutoSize = true;
            t.ColumnCount = 3;
            t.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 130));
            t.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            t.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            g.Controls.Add(t);
            return t;
        }

        private static void Fila(TableLayoutPanel t, string etiqueta, Control c, Control extra)
        {
            Label l = new Label();
            l.Text = etiqueta;
            l.AutoSize = true;
            l.Anchor = AnchorStyles.Left;
            l.Margin = new Padding(3, 7, 3, 3);
            t.Controls.Add(l, 0, t.RowCount);
            if (c is TextBox) c.Dock = DockStyle.Fill;
            c.Margin = new Padding(3, 4, 3, 3);
            t.Controls.Add(c, 1, t.RowCount);
            if (extra != null)
            {
                t.Controls.Add(extra, 2, t.RowCount);
            }
            t.RowCount++;
        }

        private static void Info(TableLayoutPanel t, Label l)
        {
            l.AutoSize = true;
            l.MaximumSize = new Size(360, 0);
            l.Margin = new Padding(3, 0, 3, 6);
            if (l.ForeColor == SystemColors.ControlText) l.ForeColor = Gris;
            t.Controls.Add(l, 1, t.RowCount);
            t.SetColumnSpan(l, 2);
            t.RowCount++;
        }

        private static void ConfigurarLista(ListView lv, string[] columnas, int[] anchos)
        {
            lv.Dock = DockStyle.Fill;
            lv.View = View.Details;
            lv.FullRowSelect = true;
            lv.GridLines = true;
            lv.HideSelection = false;
            lv.ShowItemToolTips = true;
            for (int i = 0; i < columnas.Length; i++)
            {
                lv.Columns.Add(columnas[i], anchos[i]);
            }
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
                    else if (k == "existentes") { int i2; if (int.TryParse(v, out i2) && i2 >= 0 && i2 < _rbExistentes.Length) _rbExistentes[i2].Checked = true; }
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
                File.WriteAllText(r, "listado=" + _txtListado.Text + "\r\ntipo=" + Prefijo() + "\r\nexistentes=" + IndiceExistentes() + "\r\n", Encoding.UTF8);
            }
            catch { }
        }
    }
}
