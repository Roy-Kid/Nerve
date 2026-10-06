using System.Diagnostics;
using System.Text.Json;
using Nerve.Windows;

static void Check(bool condition, string message) { if (!condition) throw new Exception(message); }
static void Reject(string json) {
    try { Protocol.Decode(json); throw new Exception("Invalid protocol was accepted"); }
    catch (InvalidDataException) { }
}
Reject("{\"version\":2,\"type\":\"frame\"}");
Reject("{\"version\":1,\"type\":\"frame\",\"iconSize\":32,\"iconRgba\":\"AA==\"}");
Reject("{\"version\":1,\"type\":\"frame\",\"iconSize\":999999,\"iconRgba\":\"\"}");
Check(JsonSerializer.Serialize(new { type = "open", id = "会话\"\\" }, Protocol.Json).Contains("open"), "Commands must remain structured JSON");

var root = Path.GetFullPath(args[0]);
var core = Path.Combine(root, "target", "debug", "nerve-windows-core.exe");
var fixture = Path.Combine(root, "fixtures", "demo_snapshot.json");
var info = new ProcessStartInfo(core) { RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false, CreateNoWindow = true };
info.ArgumentList.Add("--fixture"); info.ArgumentList.Add(fixture);
using var process = Process.Start(info)!;
_ = process.StandardError.ReadToEndAsync();
var first = Protocol.Decode((await process.StandardOutput.ReadLineAsync().WaitAsync(TimeSpan.FromSeconds(5)))!);
Check(first.Type == "frame" && first.Sections.Sum(s => s.Jobs.Length) == 2, "Rust fixture must reach the native DTO");
await process.StandardInput.WriteLineAsync("{\"type\":\"approve\",\"id\":\"demo-agent-1\"}");
var error = Protocol.Decode((await process.StandardOutput.ReadLineAsync().WaitAsync(TimeSpan.FromSeconds(5)))!);
Check(error.Type == "error", "Unsupported job-control command must be rejected");
await process.StandardInput.WriteLineAsync("{\"type\":\"cycleGroup\"}");
var grouped = Protocol.Decode((await process.StandardOutput.ReadLineAsync().WaitAsync(TimeSpan.FromSeconds(5)))!);
Check(grouped.Type == "frame" && grouped.Settings.GroupMode == "priority", "Grouping belongs to Rust");
process.StandardInput.Close();
await process.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(5));
Check(process.ExitCode == 0, "Host stdin EOF must release the Rust core");
Console.WriteLine("PASS — protocol validation, Rust projection, command rejection, grouping, and parent EOF lifecycle");
