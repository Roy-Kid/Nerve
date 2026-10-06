using System.Text.Json;

namespace Nerve.Windows;

// Presentation DTOs only. Status, grouping, actions and notification decisions
// arrive from the Rust process; this host never interprets producer text.
internal sealed record Preferences {
    public float PanelWidth { get; init; } = 340;
    public float PanelHeight { get; init; } = 360;
    public string GroupMode { get; init; } = "machine";
    public bool ToastsEnabled { get; init; }
    public bool ToastSound { get; init; }
    public string ToastFloor { get; init; } = "suggested";
    public bool Autostart { get; init; }
}
internal sealed record JobRow {
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
    public string Alias { get; init; } = "";
    public string Status { get; init; } = "inactive";
    public string Color { get; init; } = "#808080";
    public string Activity { get; init; } = "";
    public string Age { get; init; } = "";
    public string Prompt { get; init; } = "";
    public string Workspace { get; init; } = "";
    public string Location { get; init; } = "";
    public string[] Recent { get; init; } = [];
}
internal sealed record JobSection {
    public string Title { get; init; } = "";
    public JobRow[] Jobs { get; init; } = [];
}
internal sealed record DesktopMessage {
    public int Version { get; init; }
    public string Type { get; init; } = "";
    public bool Offline { get; init; }
    public int Running { get; init; }
    public int Attention { get; init; }
    public Preferences Settings { get; init; } = new();
    public JobSection[] Sections { get; init; } = [];
    public string Tooltip { get; init; } = "Nerve";
    public int IconSize { get; init; }
    public byte[] IconRgba { get; init; } = [];
    public string Id { get; init; } = "";
    public string Title { get; init; } = "";
    public string Body { get; init; } = "";
    public bool Sound { get; init; }
    public string Message { get; init; } = "";
    public bool Opened { get; init; }
}
internal static class Protocol {
    public static readonly JsonSerializerOptions Json = new() {
        PropertyNameCaseInsensitive = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };
    public static DesktopMessage Decode(string line) {
        var message = JsonSerializer.Deserialize<DesktopMessage>(line, Json) ?? throw new InvalidDataException("Empty desktop message");
        if (message.Version != 1) throw new InvalidDataException("Incompatible Rust desktop core protocol");
        if (message.Type == "frame" && (message.IconSize is < 1 or > 64 || message.IconRgba.Length != message.IconSize * message.IconSize * 4))
            throw new InvalidDataException("Invalid tray bitmap");
        return message;
    }
}
