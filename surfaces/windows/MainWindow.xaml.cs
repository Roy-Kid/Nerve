using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Windowing;
using Microsoft.Windows.AppNotifications;
using Microsoft.Win32;
using System.Runtime.InteropServices;
using System.Security;
using System.Text.Json;
using Windows.Graphics;
using Windows.Graphics.Imaging;
using Windows.Storage;
using Windows.Storage.Streams;
using Windows.System;
using Windows.UI;

namespace Nerve.Windows;

public sealed partial class MainWindow : Window {
    private readonly CoreProcess _core;
    private readonly Action _quit;
    private readonly nint _hwnd;
    private Preferences _settings = new();
    private DesktopMessage? _frame;
    private bool _applying, _quitting, _visible;
    private string? _expanded;
    private string _layout = "";
    private readonly Dictionary<string, string> _rowSignatures = new();
    private readonly DispatcherTimer _resizeTimer = new() { Interval = TimeSpan.FromMilliseconds(350) };
    private readonly DispatcherTimer _noteTimer = new() { Interval = TimeSpan.FromSeconds(4) };
    private bool _notifications;
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";

    internal MainWindow(CoreProcess core, Action quit) {
        InitializeComponent(); _core = core; _quit = quit;
        _hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var style = GetWindowLongPtr(_hwnd, -20).ToInt64();
        SetWindowLongPtr(_hwnd, -20, new nint((style | 0x80) & ~0x40000)); // tool window, no taskbar entry
        if (AppWindow.Presenter is OverlappedPresenter presenter) {
            presenter.IsMaximizable = false; presenter.IsMinimizable = false;
            presenter.IsAlwaysOnTop = true;
        }
        // Native title bar and borders supply reliable drag, resize and DPI behavior.
        AppWindow.Resize(new SizeInt32(360, 410));
        Closed += (_, args) => { if (!_quitting) { args.Handled = true; HidePanel(); } };
        Activated += (_, args) => { if (args.WindowActivationState == WindowActivationState.Deactivated && _visible) HidePanel(); };
        AppWindow.Changed += (_, args) => {
            if (args.DidSizeChange && !_applying && _visible) { _resizeTimer.Stop(); _resizeTimer.Start(); }
        };
        _resizeTimer.Tick += async (_, _) => {
            _resizeTimer.Stop();
            var scale = Root.XamlRoot?.RasterizationScale ?? 1;
            _settings = _settings with { PanelWidth = Math.Clamp((float)(AppWindow.Size.Width / scale), 320, 560), PanelHeight = Math.Clamp((float)(AppWindow.Size.Height / scale), 240, 800) };
            var bounded = new SizeInt32((int)(_settings.PanelWidth * scale), (int)(_settings.PanelHeight * scale));
            if (bounded.Width != AppWindow.Size.Width || bounded.Height != AppWindow.Size.Height) {
                _applying = true; AppWindow.Resize(bounded); _applying = false;
            }
            await SaveAsync();
        };
        _noteTimer.Tick += (_, _) => { _noteTimer.Stop(); Note.IsOpen = false; };
        try {
            AppNotificationManager.Default.NotificationInvoked += (_, args) => {
                var id = Uri.UnescapeDataString(args.Argument);
                DispatcherQueue.TryEnqueue(async () => await _core.SendAsync(new { type = "open", id }));
            };
            AppNotificationManager.Default.Register(); _notifications = true;
        } catch (Exception error) { ShowNote($"Windows notifications unavailable: {error.Message}"); }
    }

    internal void ShowPanel() {
        if (!_visible) {
            var screen = System.Windows.Forms.Screen.FromPoint(System.Drawing.Point.Empty);
            GetCursorPos(out var cursor);
            screen = System.Windows.Forms.Screen.FromPoint(new System.Drawing.Point(cursor.X, cursor.Y));
            var area = screen.WorkingArea;
            var size = AppWindow.Size;
            AppWindow.Move(new PointInt32(Math.Max(area.Left, area.Right - size.Width - 12), Math.Max(area.Top, area.Bottom - size.Height - 12)));
        }
        _visible = true;
        AppWindow.Show(); Activate();
        SetForegroundWindow(_hwnd);
    }
    internal void HidePanel() { _visible = false; AppWindow.Hide(); }
    internal void Shutdown() { _quitting = true; _resizeTimer.Stop(); _noteTimer.Stop(); if (_notifications) AppNotificationManager.Default.Unregister(); Close(); }
    internal void ShowNote(string message) { Note.Message = message; Note.Severity = InfoBarSeverity.Informational; Note.IsClosable = true; Note.IsOpen = true; _noteTimer.Stop(); _noteTimer.Start(); }
    internal void ShowFailure(string message) { _noteTimer.Stop(); Counts.Text = "Core offline"; Note.Message = message; Note.Severity = InfoBarSeverity.Error; Note.IsClosable = false; Note.IsOpen = true; }

    internal void Apply(DesktopMessage frame) {
        var initial = _frame is null;
        _frame = frame; _settings = frame.Settings;
        _applying = true;
        Counts.Text = $"{frame.Running} running" + (frame.Attention > 0 ? $" · {frame.Attention} need you" : "") + (frame.Offline ? " · offline" : "");
        Toasts.IsOn = _settings.ToastsEnabled; Sound.IsOn = _settings.ToastSound;
        using (var key = Registry.CurrentUser.OpenSubKey(RunKey)) Autostart.IsOn = key?.GetValue("Nerve") is not null;
        if (initial) {
            var scale = Root.XamlRoot?.RasterizationScale ?? 1;
            AppWindow.Resize(new SizeInt32((int)(_settings.PanelWidth * scale), (int)(_settings.PanelHeight * scale)));
        }
        var selected = (Jobs.SelectedItem as ListViewItem)?.Tag as string;
        var offset = FindScrollViewer(Jobs)?.VerticalOffset ?? 0;
        var layout = JsonSerializer.Serialize(frame.Sections.Select(s => new { s.Title, ids = s.Jobs.Select(row => row.Id) }));
        if (layout != _layout) {
            _layout = layout; Jobs.Items.Clear(); _rowSignatures.Clear();
            foreach (var section in frame.Sections) {
                if (section.Title.Length > 0) Jobs.Items.Add(new ListViewItem {
                    IsEnabled = false, Content = new TextBlock { Text = section.Title.ToUpperInvariant(), FontSize = 11, Margin = new Thickness(0, 8, 0, 0) },
                });
                foreach (var row in section.Jobs) {
                    var item = new ListViewItem { Tag = row.Id, HorizontalContentAlignment = HorizontalAlignment.Stretch, Content = BuildRow(row) };
                    Jobs.Items.Add(item); _rowSignatures[row.Id] = JsonSerializer.Serialize(row);
                    if (row.Id == selected) Jobs.SelectedItem = item;
                }
            }
            FindScrollViewer(Jobs)?.ChangeView(null, offset, null, true);
        } else {
            var rows = frame.Sections.SelectMany(s => s.Jobs).ToDictionary(row => row.Id);
            foreach (var item in Jobs.Items.OfType<ListViewItem>()) {
                if (item.Tag is not string id || !rows.TryGetValue(id, out var row)) continue;
                var signature = JsonSerializer.Serialize(row);
                if (_rowSignatures.GetValueOrDefault(id) != signature) { item.Content = BuildRow(row); _rowSignatures[id] = signature; }
            }
        }
        if (_expanded is not null && !frame.Sections.SelectMany(s => s.Jobs).Any(row => row.Id == _expanded)) _expanded = null;
        EmptyTitle.Text = frame.Offline ? "Connecting to Nerve" : "Nothing running";
        EmptyMessage.Text = frame.Offline ? "Reconnecting automatically. Your jobs will appear when the connection returns." : "Start an agent with the Nerve plugin to see its progress here.";
        UpdateVisibility(); _applying = false;
    }

    private StackPanel BuildRow(JobRow row) {
        var content = new StackPanel { Spacing = 4, Margin = new Thickness(0, 4, 0, 4) };
        var title = new Grid { ColumnSpacing = 8 };
        title.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        title.ColumnDefinitions.Add(new ColumnDefinition());
        title.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var color = row.Color.TrimStart('#');
        var dot = new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 10, Height = 10, VerticalAlignment = VerticalAlignment.Center,
            Fill = new SolidColorBrush(Color.FromArgb(255, Convert.ToByte(color[..2], 16), Convert.ToByte(color[2..4], 16), Convert.ToByte(color[4..6], 16))) };
        title.Children.Add(dot);
        var name = new TextBlock { Text = row.Name, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 13 }; Grid.SetColumn(name, 1); title.Children.Add(name);
        ToolTipService.SetToolTip(name, row.Name);
        var age = new TextBlock { Text = row.Age == "-" ? "" : row.Age, FontSize = 11 }; Grid.SetColumn(age, 2); title.Children.Add(age);
        content.Children.Add(title);
        if (row.Activity.Length > 0) content.Children.Add(new TextBlock { Text = row.Activity, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 11, Opacity = .7 });
        if (_expanded == row.Id) {
            foreach (var (label, value) in new[] { ("Prompt", row.Prompt), ("Machine", row.Alias), ("Project", row.Workspace), ("Location", row.Location) }) {
                if (value.Length == 0) continue;
                content.Children.Add(new TextBlock { Text = label, FontSize = 11, Opacity = .65 });
                content.Children.Add(new TextBlock { Text = value, TextWrapping = TextWrapping.Wrap, FontSize = 12, IsTextSelectionEnabled = true });
            }
            var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            foreach (var verb in new[] { "Open", "Copy" }) {
                var button = new Button { Content = verb };
                button.Click += async (_, _) => await _core.SendAsync(new { type = verb.ToLowerInvariant(), id = row.Id });
                actions.Children.Add(button);
            }
            content.Children.Add(actions);
            foreach (var entry in row.Recent) content.Children.Add(new TextBlock { Text = entry, TextWrapping = TextWrapping.Wrap, FontSize = 11, Opacity = .7 });
        }
        return content;
    }
    private void JobClicked(object sender, ItemClickEventArgs args) {
        if (args.ClickedItem is not ListViewItem { Tag: string id }) return;
        ToggleRow(id);
    }
    private void ToggleRow(string id) {
        _expanded = _expanded == id ? null : id;
        foreach (var item in Jobs.Items.OfType<ListViewItem>()) {
            if (item.Tag is not string jobId) continue;
            var row = _frame?.Sections.SelectMany(section => section.Jobs).FirstOrDefault(row => row.Id == jobId);
            if (row is not null) item.Content = BuildRow(row);
        }
    }
    private void OnKeyDown(object sender, KeyRoutedEventArgs args) {
        if (args.Key == VirtualKey.Escape) { HidePanel(); args.Handled = true; }
        // Native ListView ItemClick handles mouse and Enter/Space activation;
        // it also owns arrow navigation, focus and scrolling.
    }
    private async void CycleGroup(object sender, RoutedEventArgs args) => await _core.SendAsync(new { type = "cycleGroup" });
    private void ToggleSettings(object sender, RoutedEventArgs args) => UpdateVisibility();
    private void QuitClicked(object sender, RoutedEventArgs args) => _quit();
    private void UpdateVisibility() {
        var settings = SettingsButton.IsChecked == true;
        SettingsScroll.Visibility = settings ? Visibility.Visible : Visibility.Collapsed;
        var empty = _frame?.Sections.All(section => section.Jobs.Length == 0) ?? true;
        Empty.Visibility = !settings && empty ? Visibility.Visible : Visibility.Collapsed;
        Jobs.Visibility = !settings && !empty ? Visibility.Visible : Visibility.Collapsed;
    }
    private async void PreferencesChanged(object sender, RoutedEventArgs args) {
        if (_applying) return;
        try {
            if (ReferenceEquals(sender, Autostart) && App.Argument("--fixture") is null) {
                using var key = Registry.CurrentUser.CreateSubKey(RunKey);
                if (Autostart.IsOn) key.SetValue("Nerve", $"\"{Environment.ProcessPath}\" --autostart");
                else key.DeleteValue("Nerve", false);
            }
            _settings = _settings with { ToastsEnabled = Toasts.IsOn, ToastSound = Sound.IsOn, Autostart = Autostart.IsOn };
            await SaveAsync();
        } catch (Exception error) when (error is UnauthorizedAccessException or IOException or System.Security.SecurityException) { ShowNote(error.Message); }
    }
    private Task SaveAsync() => _core.SendAsync(new { type = "preferences", settings = _settings });
    internal void Notify(DesktopMessage message) {
        if (!_notifications) return;
        try {
            var argument = SecurityElement.Escape(Uri.EscapeDataString(message.Id));
            var xml = $"<toast launch=\"{argument}\"><visual><binding template=\"ToastGeneric\"><text>{SecurityElement.Escape(message.Title)}</text><text>{SecurityElement.Escape(message.Body)}</text></binding></visual>" + (message.Sound ? "" : "<audio silent=\"true\"/>") + "</toast>";
            AppNotificationManager.Default.Show(new AppNotification(xml));
        } catch (Exception error) { ShowNote($"Windows notification failed: {error.Message}"); }
    }
    internal async Task SaveSmokeAsync(string path, bool trayVisible) {
        if (_frame is null) throw new InvalidOperationException("Native UI did not receive a Rust frame");
        var count = _frame.Sections.Sum(section => section.Jobs.Length);
        await CaptureAsync(path + ".png");
        if (_frame.Sections.SelectMany(section => section.Jobs).FirstOrDefault() is { } first) {
            ToggleRow(first.Id); await Task.Delay(200); await CaptureAsync(path + ".expanded.png");
        }
        SettingsButton.IsChecked = true; UpdateVisibility(); await Task.Delay(200); await CaptureAsync(path + ".settings.png");
        SettingsScroll.ChangeView(null, SettingsScroll.ScrollableHeight, null, true);
        await Task.Delay(200); await CaptureAsync(path + ".settings-bottom.png");
        Root.RequestedTheme = ElementTheme.Dark; await Task.Delay(200); await CaptureAsync(path + ".dark.png");
        SettingsButton.IsChecked = false; Root.RequestedTheme = ElementTheme.Light;
        Apply(_frame with { Sections = [] }); await Task.Delay(200); await CaptureAsync(path + ".empty.png");
        await File.WriteAllTextAsync(path, JsonSerializer.Serialize(new { framework = "WinUI3", trayVisible, jobs = count, nativeTitleBar = !ExtendsContentIntoTitleBar, width = AppWindow.Size.Width, height = AppWindow.Size.Height }, Protocol.Json));
    }
    private async Task CaptureAsync(string path) {
        var image = new RenderTargetBitmap(); await image.RenderAsync(Root);
        var file = await StorageFile.GetFileFromPathAsync(CreateEmptyFile(path));
        using (var output = await file.OpenAsync(FileAccessMode.ReadWrite)) {
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, output);
            var pixels = await image.GetPixelsAsync();
            using var reader = DataReader.FromBuffer(pixels); var bytes = new byte[pixels.Length]; reader.ReadBytes(bytes);
            encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, (uint)image.PixelWidth, (uint)image.PixelHeight, 96, 96, bytes);
            await encoder.FlushAsync();
        }
    }
    private static string CreateEmptyFile(string path) { Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!); File.WriteAllBytes(path, []); return Path.GetFullPath(path); }
    private static ScrollViewer? FindScrollViewer(DependencyObject parent) {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++) {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is ScrollViewer scroll) return scroll;
            if (FindScrollViewer(child) is { } nested) return nested;
        }
        return null;
    }
    [StructLayout(LayoutKind.Sequential)] private struct Point { public int X, Y; }
    [DllImport("user32.dll")] private static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(nint hwnd);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] private static extern nint GetWindowLongPtr(nint hwnd, int index);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")] private static extern nint SetWindowLongPtr(nint hwnd, int index, nint value);
}
