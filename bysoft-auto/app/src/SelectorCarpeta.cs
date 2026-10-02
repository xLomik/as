// Dialogo para elegir la carpeta destino dentro de la base Parts de BySoft.
// Muestra el arbol de carpetas registradas en el indice (las mismas que ve BySoft)
// y permite escribir al final el nombre de una carpeta nueva (se crea al ejecutar).

using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;

namespace AutoBySoft
{
    public sealed class SelectorCarpeta : Form
    {
        private const string Pendiente = "\u0001";
        private readonly BySoftApi _api;
        private readonly TreeView _arbol = new TreeView();
        private readonly TextBox _txtRuta = new TextBox();

        public string Ruta { get { return _txtRuta.Text.Trim().Trim('\\', '/'); } }

        public SelectorCarpeta(BySoftApi api, string rutaInicial)
        {
            _api = api;
            Text = "Elegir carpeta destino en BySoft (Parts)";
            Font = new Font("Segoe UI", 9f);
            ClientSize = new Size(560, 520);
            MinimumSize = new Size(420, 380);
            StartPosition = FormStartPosition.CenterParent;
            MinimizeBox = false;
            MaximizeBox = false;
            ShowIcon = false;

            Label ayuda = new Label();
            ayuda.Dock = DockStyle.Top;
            ayuda.Height = 44;
            ayuda.Padding = new Padding(8, 6, 8, 0);
            ayuda.Text = "Elige la carpeta del proyecto. Para crear una carpeta nueva, elige la carpeta padre " +
                         "y escribe el nombre nuevo al final de la ruta (se creara al ejecutar).";

            _arbol.Dock = DockStyle.Fill;
            _arbol.HideSelection = false;
            _arbol.BeforeExpand += delegate (object s, TreeViewCancelEventArgs e) { Cargar(e.Node); };
            _arbol.AfterSelect += delegate (object s, TreeViewEventArgs e) { _txtRuta.Text = RutaDe(e.Node); };
            _arbol.NodeMouseDoubleClick += delegate (object s, TreeNodeMouseClickEventArgs e) { DialogResult = DialogResult.OK; };

            TableLayoutPanel abajo = new TableLayoutPanel();
            abajo.Dock = DockStyle.Bottom;
            abajo.Height = 80;
            abajo.Padding = new Padding(8);
            abajo.ColumnCount = 2;
            abajo.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            abajo.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            Label lr = new Label();
            lr.Text = "Ruta:";
            lr.AutoSize = true;
            lr.Margin = new Padding(3, 7, 3, 3);
            _txtRuta.Dock = DockStyle.Fill;
            _txtRuta.Text = rutaInicial ?? "";
            abajo.Controls.Add(lr, 0, 0);
            abajo.Controls.Add(_txtRuta, 1, 0);
            FlowLayoutPanel botones = new FlowLayoutPanel();
            botones.FlowDirection = FlowDirection.RightToLeft;
            botones.Dock = DockStyle.Fill;
            Button cancelar = new Button();
            cancelar.Text = "Cancelar";
            cancelar.DialogResult = DialogResult.Cancel;
            cancelar.AutoSize = true;
            Button aceptar = new Button();
            aceptar.Text = "Aceptar";
            aceptar.DialogResult = DialogResult.OK;
            aceptar.AutoSize = true;
            botones.Controls.Add(cancelar);
            botones.Controls.Add(aceptar);
            abajo.Controls.Add(botones, 0, 1);
            abajo.SetColumnSpan(botones, 2);
            AcceptButton = aceptar;
            CancelButton = cancelar;

            Controls.Add(_arbol);
            Controls.Add(abajo);
            Controls.Add(ayuda);

            Shown += delegate { Inicio(rutaInicial); };
        }

        private void Inicio(string rutaInicial)
        {
            TreeNode raiz = new TreeNode("Parts");
            raiz.Tag = "";
            raiz.Nodes.Add(Pendiente);
            _arbol.Nodes.Add(raiz);
            raiz.Expand();
            // Abre el arbol hasta la ruta que ya estaba escrita (lo que exista).
            TreeNode actual = raiz;
            foreach (string parte in BySoftApi.Partes(rutaInicial))
            {
                TreeNode sig = null;
                foreach (TreeNode n in actual.Nodes)
                {
                    if (string.Equals(n.Text, parte, StringComparison.OrdinalIgnoreCase))
                    {
                        sig = n;
                        break;
                    }
                }
                if (sig == null)
                {
                    break;
                }
                sig.Expand();
                actual = sig;
            }
            _arbol.SelectedNode = actual;
            _txtRuta.Text = rutaInicial ?? "";
            actual.EnsureVisible();
        }

        private void Cargar(TreeNode nodo)
        {
            if (nodo.Nodes.Count != 1 || nodo.Nodes[0].Text != Pendiente)
            {
                return;
            }
            nodo.Nodes.Clear();
            Cursor = Cursors.WaitCursor;
            try
            {
                List<string> hijos = _api.Subcarpetas("Parts", (string)nodo.Tag);
                foreach (string h in hijos)
                {
                    TreeNode n = new TreeNode(h);
                    n.Tag = ((string)nodo.Tag).Length == 0 ? h : (string)nodo.Tag + "\\" + h;
                    n.Nodes.Add(Pendiente);
                    nodo.Nodes.Add(n);
                }
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "No se pudieron leer las carpetas de BySoft:\n" + ex.Message, Text);
            }
            finally
            {
                Cursor = Cursors.Default;
            }
        }

        private static string RutaDe(TreeNode n)
        {
            return n == null ? "" : (string)n.Tag;
        }
    }
}
