using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using Forms = System.Windows.Forms;

namespace Nerve.Windows;

// NotifyIcon wraps Windows Shell_NotifyIcon and its native message window.
// All task/status decisions and bitmap geometry come from the Rust binary.
internal sealed class Tray : IDisposable {
    private readonly Forms.NotifyIcon _icon;
    private Icon? _image;
    private string _lastBitmap = "";
    public Tray(Action open, Action quit) {
        var menu = new Forms.ContextMenuStrip();
        menu.Items.Add("Open Nerve", null, (_, _) => open());
        menu.Items.Add(new Forms.ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => quit());
        _icon = new Forms.NotifyIcon { Text = "Nerve", Icon = SystemIcons.Application, ContextMenuStrip = menu, Visible = true };
        _icon.MouseClick += (_, args) => { if (args.Button == Forms.MouseButtons.Left) open(); };
    }
    public void Update(DesktopMessage message) {
        _icon.Text = message.Tooltip.Length > 127 ? message.Tooltip[..127] : message.Tooltip;
        var signature = Convert.ToBase64String(message.IconRgba);
        if (signature == _lastBitmap) return;
        using var bitmap = new Bitmap(message.IconSize, message.IconSize, PixelFormat.Format32bppArgb);
        var data = bitmap.LockBits(new Rectangle(0, 0, bitmap.Width, bitmap.Height), ImageLockMode.WriteOnly, bitmap.PixelFormat);
        try {
            var bgra = (byte[])message.IconRgba.Clone();
            for (var at = 0; at < bgra.Length; at += 4) (bgra[at], bgra[at + 2]) = (bgra[at + 2], bgra[at]);
            Marshal.Copy(bgra, 0, data.Scan0, bgra.Length);
        } finally { bitmap.UnlockBits(data); }
        var handle = bitmap.GetHicon();
        try {
            using var borrowed = Icon.FromHandle(handle);
            var replacement = (Icon)borrowed.Clone();
            _icon.Icon = replacement;
            _image?.Dispose();
            _image = replacement;
            _lastBitmap = signature;
        } finally { DestroyIcon(handle); }
    }
    public bool Visible => _icon.Visible;
    public void Dispose() { _icon.Visible = false; _icon.ContextMenuStrip?.Dispose(); _icon.Dispose(); _image?.Dispose(); }
    [DllImport("user32.dll")] private static extern bool DestroyIcon(nint icon);
}
