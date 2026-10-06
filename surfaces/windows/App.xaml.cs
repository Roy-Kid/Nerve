using Microsoft.UI.Xaml;
using System.Net;
using System.Net.Sockets;

namespace Nerve.Windows;

public partial class App : Application {
    private MainWindow? _window;
    private TcpListener? _listener;
    private readonly CancellationTokenSource _stop = new();
    private CoreProcess? _core;
    private Tray? _tray;
    private bool _quitting;
    internal static App CurrentApp => (App)Current;
    public App() => InitializeComponent();
    internal static string? Argument(string option) {
        var args = Environment.GetCommandLineArgs(); var at = Array.IndexOf(args, option);
        return at >= 0 && at + 1 < args.Length ? args[at + 1] : null;
    }
    protected override async void OnLaunched(LaunchActivatedEventArgs args) {
        var autostart = Environment.GetCommandLineArgs().Contains("--autostart");
        if (autostart) await Task.Delay(4000);
        // Same lock as the legacy surface, so migration cannot attach two streams.
        try {
            _listener = new TcpListener(IPAddress.Loopback, Argument("--smoke") is null ? 17891 : 0);
            _listener.Server.ExclusiveAddressUse = true;
            _listener.Start(16);
        } catch (SocketException) {
            if (!autostart) {
                try { using var client = new TcpClient(); await client.ConnectAsync(IPAddress.Loopback, 17891); }
                catch (SocketException) { }
            }
            Exit(); return;
        }
        _core = new CoreProcess();
        _window = new MainWindow(_core, Quit);
        _tray = new Tray(() => _window.ShowPanel(), Quit);
        _core.Message += message => _window.DispatcherQueue.TryEnqueue(() => {
            if (message.Type == "frame") { _tray.Update(message); _window.Apply(message); }
            else if (message.Type == "toast") _window.Notify(message);
            else if (message.Type == "action") { _window.ShowNote(message.Message); if (message.Opened) _window.HidePanel(); }
            else _window.ShowNote(message.Message);
        });
        _core.Failed += error => _window.DispatcherQueue.TryEnqueue(() => _window.ShowFailure(error));
        try { _core.Start(Argument("--fixture")); }
        catch (Exception error) when (error is IOException or System.ComponentModel.Win32Exception) { _window.ShowFailure(error.Message); }
        _ = ListenAsync();
        if (!autostart) _window.ShowPanel();
        if (Argument("--smoke") is { } path) {
            await Task.Delay(2500);
            await _window.SaveSmokeAsync(path, _tray.Visible);
            Quit();
        }
    }
    private async Task ListenAsync() {
        try {
            while (!_stop.IsCancellationRequested && _listener is not null) {
                using var client = await _listener.AcceptTcpClientAsync(_stop.Token);
                _window?.DispatcherQueue.TryEnqueue(() => _window.ShowPanel());
            }
        } catch (Exception error) when (error is OperationCanceledException or SocketException or ObjectDisposedException) { }
    }
    internal void Quit() {
        if (_quitting) return;
        _quitting = true;
        _stop.Cancel(); _listener?.Stop();
        _tray?.Dispose(); _core?.Dispose();
        _window?.Shutdown();
        Exit();
    }
}
