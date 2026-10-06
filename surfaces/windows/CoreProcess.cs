using System.Diagnostics;
using System.Text;
using System.Text.Json;

namespace Nerve.Windows;

internal sealed class CoreProcess : IDisposable {
    private Process? _process;
    private readonly SemaphoreSlim _writer = new(1);
    private bool _disposed;
    public event Action<DesktopMessage>? Message;
    public event Action<string>? Failed;
    public void Start(string? fixture) {
        var path = Path.Combine(AppContext.BaseDirectory, "nerve-windows-core.exe");
        if (!File.Exists(path)) throw new FileNotFoundException("Install the Rust desktop core beside the Windows UI.", path);
        var info = new ProcessStartInfo(path) {
            UseShellExecute = false, CreateNoWindow = true,
            RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true,
            StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8,
        };
        if (fixture is not null) { info.ArgumentList.Add("--fixture"); info.ArgumentList.Add(fixture); }
        _process = Process.Start(info) ?? throw new IOException("Rust core did not start");
        _process.StandardInput.AutoFlush = true;
        _ = ReadAsync(_process);
        _ = DrainErrorsAsync(_process);
    }
    private async Task ReadAsync(Process process) {
        try {
            while (await process.StandardOutput.ReadLineAsync() is { } line) {
                Message?.Invoke(Protocol.Decode(line));
            }
            if (!_disposed) Failed?.Invoke("The Rust desktop core stopped. Restart Nerve to reconnect.");
        } catch (Exception error) when (error is IOException or JsonException or InvalidDataException or ObjectDisposedException) {
            if (!_disposed) Failed?.Invoke(error.Message);
        }
    }
    private static async Task DrainErrorsAsync(Process process) {
        try { while (await process.StandardError.ReadLineAsync() is not null) { } }
        catch (Exception error) when (error is IOException or ObjectDisposedException) { }
    }
    public async Task SendAsync(object command) {
        await _writer.WaitAsync();
        try {
            if (_disposed || _process is null || _process.HasExited) return;
            await _process.StandardInput.WriteLineAsync(JsonSerializer.Serialize(command, Protocol.Json));
        } catch (IOException error) { if (!_disposed) Failed?.Invoke(error.Message); }
        finally { _writer.Release(); }
    }
    public void Dispose() {
        _disposed = true;
        if (_process is not { } process) return;
        // Closing stdin is the core's shutdown signal, including when the host crashes.
        try { process.StandardInput.Close(); if (!process.WaitForExit(1500)) process.Kill(); }
        catch (Exception error) when (error is InvalidOperationException or IOException) { }
        process.Dispose();
    }
}
