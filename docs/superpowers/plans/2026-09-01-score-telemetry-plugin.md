# Score Telemetry Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and live-verify a minimal Revolution Idle BepInEx plugin that sends the current score to a configurable localhost UDP port every 50 ms.

**Architecture:** Framework-independent source files format a `BigDouble` mantissa/exponent pair as UTF-8 JSON and send it through a silent, one-way UDP publisher. A BepInEx 6 IL2CPP entry point binds the port, attaches one persistent `MonoBehaviour`, and reads `GameController.data.score` on Unity's main thread whenever an unscaled 50 ms accumulator elapses.

**Tech Stack:** C#; .NET 9 SDK for development/tests; `net6.0` plugin target; BepInEx 6.0.0-be.785 IL2CPP; generated Revolution Idle IL2CPP interop assemblies; Windows UDP sockets.

**Spec:** `docs/superpowers/specs/2026-09-01-score-telemetry-plugin-design.md`

## Global Constraints

- All plugin source lives under `plugin/`; the repository root remains available for a future client.
- Target BepInEx 6 for the Windows x64 IL2CPP build of Revolution Idle.
- Run game access on Unity's main thread and read `GameController.data.score` without changing game state.
- Bind the BepInEx configuration key `[Network] Port` with default value `19841`.
- Port `0`, non-numeric values, and values outside `1..65535` disable publishing.
- Send only to `127.0.0.1:<Port>`.
- Send at most one datagram per Unity frame whenever a 50 ms timer based on `Time.unscaledDeltaTime` elapses.
- Encode exactly one UTF-8 JSON property: `{"score":"<mantissa>e<exponent>"}`.
- Format both generated `double` properties, `BigDouble.Mantissa` and `BigDouble.Exponent`, with invariant round-trip (`"R"`) formatting; do not convert the combined score through `double`.
- Silently ignore missing game state and all read, serialization, socket-construction, UDP-send, and socket-disposal failures.
- Do not add UI, a receiver application, retries, acknowledgements, state mutation, scripting, mouse automation, or routine network-failure logging.
- The current local interop contract is `GameController.data : GameData`, `GameData.score : BigDouble`, `BigDouble.Mantissa : double`, and `BigDouble.Exponent : double`.

---

### Task 1: Payload Encoding and Silent Localhost UDP Publisher

**Files:**
- Create: `plugin/src/ScorePayload.cs`
- Create: `plugin/src/UdpScorePublisher.cs`
- Create: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Create: `plugin/tests/Program.cs`

**Interfaces:**
- Consumes: no project code; only .NET `System.Globalization`, `System.Net`, `System.Net.Sockets`, and `System.Text` APIs.
- Produces: `ScorePayload.Encode(double mantissa, double exponent) : byte[]`, `UdpScorePublisher.Create(string? configuredPort) : UdpScorePublisher?`, `UdpScorePublisher.Publish(byte[] payload) : void`, and `UdpScorePublisher.Dispose() : void`.

- [ ] **Step 1: Write the failing executable tests**

Create `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net9.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
  </PropertyGroup>

  <ItemGroup>
    <Compile Include="Program.cs" />
    <Compile Include="..\src\ScorePayload.cs" Link="ScorePayload.cs" />
    <Compile Include="..\src\UdpScorePublisher.cs" Link="UdpScorePublisher.cs" />
  </ItemGroup>
</Project>
```

Create `plugin/tests/Program.cs`:

```csharp
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using RevIdle.ScoreTelemetry;

await ScorePayloadUsesInvariantRoundTripFormatting();
InvalidPortsDisablePublishing();
await PublisherSendsExactPayloadToLoopback();
Console.WriteLine("3 tests passed.");

static Task ScorePayloadUsesInvariantRoundTripFormatting()
{
    CultureInfo previous = CultureInfo.CurrentCulture;
    CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("fr-FR");
    try
    {
        string actual = Encoding.UTF8.GetString(ScorePayload.Encode(1.2345678901234567, 123));
        Equal("{\"score\":\"1.2345678901234567e123\"}", actual, nameof(ScorePayloadUsesInvariantRoundTripFormatting));
    }
    finally
    {
        CultureInfo.CurrentCulture = previous;
    }

    return Task.CompletedTask;
}

static void InvalidPortsDisablePublishing()
{
    foreach (string? value in new string?[] { null, "", "not-a-port", "0", "-1", "65536" })
    {
        using UdpScorePublisher? publisher = UdpScorePublisher.Create(value);
        if (publisher is not null)
            throw new InvalidOperationException($"{nameof(InvalidPortsDisablePublishing)}: '{value}' should be disabled.");
    }
}

static async Task PublisherSendsExactPayloadToLoopback()
{
    using var receiver = new UdpClient(new IPEndPoint(IPAddress.Loopback, 0));
    int port = ((IPEndPoint)receiver.Client.LocalEndPoint!).Port;
    using UdpScorePublisher publisher = UdpScorePublisher.Create(port.ToString(CultureInfo.InvariantCulture))
        ?? throw new InvalidOperationException($"{nameof(PublisherSendsExactPayloadToLoopback)}: valid port was rejected.");
    byte[] payload = Encoding.UTF8.GetBytes("{\"score\":\"2.5e42\"}");

    publisher.Publish(payload);

    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(2));
    UdpReceiveResult result = await receiver.ReceiveAsync(timeout.Token);
    Equal("127.0.0.1", result.RemoteEndPoint.Address.ToString(), nameof(PublisherSendsExactPayloadToLoopback));
    Equal("{\"score\":\"2.5e42\"}", Encoding.UTF8.GetString(result.Buffer), nameof(PublisherSendsExactPayloadToLoopback));
}

static void Equal<T>(T expected, T actual, string testName)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
        throw new InvalidOperationException($"{testName}: expected '{expected}', got '{actual}'.");
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
```

Expected: compilation fails because `ScorePayload` and `UdpScorePublisher` do not exist. Confirm the failure names those missing production APIs rather than a malformed test project.

- [ ] **Step 3: Implement invariant score payload encoding**

Create `plugin/src/ScorePayload.cs`:

```csharp
using System.Globalization;
using System.Text;

namespace RevIdle.ScoreTelemetry;

internal static class ScorePayload
{
    public static byte[] Encode(double mantissa, double exponent)
    {
        string score = string.Concat(
            mantissa.ToString("R", CultureInfo.InvariantCulture),
            "e",
            exponent.ToString("R", CultureInfo.InvariantCulture));

        return Encoding.UTF8.GetBytes(string.Concat("{\"score\":\"", score, "\"}"));
    }
}
```

- [ ] **Step 4: Implement port validation and silent UDP publishing**

Create `plugin/src/UdpScorePublisher.cs`:

```csharp
using System.Globalization;
using System.Net;
using System.Net.Sockets;

namespace RevIdle.ScoreTelemetry;

internal sealed class UdpScorePublisher : IDisposable
{
    private readonly UdpClient _client;
    private readonly IPEndPoint _destination;

    private UdpScorePublisher(int port)
    {
        _client = new UdpClient(AddressFamily.InterNetwork);
        _destination = new IPEndPoint(IPAddress.Loopback, port);
    }

    public static UdpScorePublisher? Create(string? configuredPort)
    {
        if (!int.TryParse(configuredPort, NumberStyles.None, CultureInfo.InvariantCulture, out int port)
            || port is < 1 or > 65535)
            return null;

        try
        {
            return new UdpScorePublisher(port);
        }
        catch
        {
            return null;
        }
    }

    public void Publish(byte[] payload)
    {
        try
        {
            _client.Send(payload, payload.Length, _destination);
        }
        catch
        {
        }
    }

    public void Dispose()
    {
        try
        {
            _client.Dispose();
        }
        catch
        {
        }
    }
}
```

- [ ] **Step 5: Run the tests and verify GREEN**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
```

Expected: exit code `0` and the exact final line `3 tests passed.`

- [ ] **Step 6: Commit Task 1**

```powershell
git add plugin/src/ScorePayload.cs plugin/src/UdpScorePublisher.cs plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj plugin/tests/Program.cs
git commit -m "feat: add score UDP payload publisher"
```

---

### Task 2: BepInEx Integration, Build Documentation, and Live Packet Verification

**Files:**
- Create: `plugin/src/RevolutionIdle.ScoreTelemetry.csproj`
- Create: `plugin/src/Plugin.cs`
- Modify: `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`
- Modify: `plugin/tests/Program.cs`
- Create: `plugin/README.md`

**Interfaces:**
- Consumes: all Task 1 interfaces; generated interop properties `GameController.data`, `GameData.score`, `BigDouble.Mantissa`, and `BigDouble.Exponent`; BepInEx `BasePlugin.AddComponent<T>()`; Unity `Time.unscaledDeltaTime`.
- Produces: BepInEx plugin GUID `dev.tnthung.revolutionidle.scoretelemetry`; `[Network] Port` string setting; `ScoreTicker.AdvanceTimer(ref float elapsed, float delta) : bool`; Release artifact `plugin/src/bin/Release/RevIdle.ScoreTelemetry.dll`.

- [ ] **Step 1: Add the failing 50 ms accumulator test**

Add this line before `Console.WriteLine("3 tests passed.");` in `plugin/tests/Program.cs`, and change that final message to `4 tests passed.`:

```csharp
TickerUsesFiftyMillisecondAccumulator();
```

Add this test before `Equal<T>`:

```csharp
static void TickerUsesFiftyMillisecondAccumulator()
{
    float elapsed = 0f;
    Equal(false, ScoreTicker.AdvanceTimer(ref elapsed, 0.049f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Equal(true, ScoreTicker.AdvanceTimer(ref elapsed, 0.001f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Near(0f, elapsed, nameof(TickerUsesFiftyMillisecondAccumulator));
    Equal(true, ScoreTicker.AdvanceTimer(ref elapsed, 0.12f), nameof(TickerUsesFiftyMillisecondAccumulator));
    Near(0.02f, elapsed, nameof(TickerUsesFiftyMillisecondAccumulator));
}

static void Near(float expected, float actual, string testName)
{
    if (MathF.Abs(expected - actual) > 0.0001f)
        throw new InvalidOperationException($"{testName}: expected approximately '{expected}', got '{actual}'.");
}
```

Add these items inside the existing `<ItemGroup>` in `plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj`:

```xml
<Compile Include="..\src\Plugin.cs" Link="Plugin.cs" />
```

Add this property inside `<PropertyGroup>`:

```xml
<GameDir Condition="'$(GameDir)' == ''">C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle</GameDir>
```

Add this second item group after the compile item group:

```xml
<ItemGroup>
  <Reference Include="$([System.IO.Directory]::GetFiles('$(GameDir)\BepInEx\core', '*.dll'))">
    <Private>true</Private>
  </Reference>
  <Reference Include="$([System.IO.Directory]::GetFiles('$(GameDir)\BepInEx\interop', '*.dll'))">
    <Private>true</Private>
  </Reference>
</ItemGroup>
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
```

Expected: compilation fails because `Plugin.cs` and `ScoreTicker.AdvanceTimer` do not yet exist. Confirm the failure is caused by the missing production API.

- [ ] **Step 3: Create the net6.0 IL2CPP plugin project**

Create `plugin/src/RevolutionIdle.ScoreTelemetry.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
    <CheckEolTargetFramework>false</CheckEolTargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <AssemblyName>RevIdle.ScoreTelemetry</AssemblyName>
    <RootNamespace>RevIdle.ScoreTelemetry</RootNamespace>
    <Version>0.1.0</Version>
    <AppendTargetFrameworkToOutputPath>false</AppendTargetFrameworkToOutputPath>
    <GameDir Condition="'$(GameDir)' == ''">C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle</GameDir>
    <NoWarn>$(NoWarn);CS0436;MSB3277;CS0618</NoWarn>
  </PropertyGroup>

  <ItemGroup>
    <Reference Include="$([System.IO.Directory]::GetFiles('$(GameDir)\BepInEx\core', '*.dll'))">
      <Private>false</Private>
    </Reference>
    <Reference Include="$([System.IO.Directory]::GetFiles('$(GameDir)\BepInEx\interop', '*.dll'))">
      <Private>false</Private>
    </Reference>
  </ItemGroup>
</Project>
```

- [ ] **Step 4: Implement the BepInEx entry point and unscaled ticker**

Create `plugin/src/Plugin.cs`:

```csharp
using BepInEx;
using BepInEx.Unity.IL2CPP;
using UnityEngine;

namespace RevIdle.ScoreTelemetry;

[BepInPlugin(PluginGuid, PluginName, PluginVersion)]
public sealed class Plugin : BasePlugin
{
    public const string PluginGuid = "dev.tnthung.revolutionidle.scoretelemetry";
    public const string PluginName = "Revolution Idle Score Telemetry";
    public const string PluginVersion = "0.1.0";

    private static UdpScorePublisher? _publisher;

    public override void Load()
    {
        string configuredPort = Config.Bind(
            "Network",
            "Port",
            "19841",
            "UDP destination port on 127.0.0.1. Set to 0 to disable.").Value;

        _publisher = UdpScorePublisher.Create(configuredPort);
        if (_publisher is not null)
            AddComponent<ScoreTicker>();
    }

    internal static void PublishScore()
    {
        try
        {
            GameData? data = GameController.data;
            if (data is null || _publisher is null)
                return;

            BigDouble score = data.score;
            _publisher.Publish(ScorePayload.Encode(score.Mantissa, score.Exponent));
        }
        catch
        {
        }
    }
}

public sealed class ScoreTicker : MonoBehaviour
{
    private const float IntervalSeconds = 0.05f;
    private float _elapsed;

    public ScoreTicker(IntPtr pointer) : base(pointer)
    {
    }

    public void Update()
    {
        if (AdvanceTimer(ref _elapsed, Time.unscaledDeltaTime))
            Plugin.PublishScore();
    }

    internal static bool AdvanceTimer(ref float elapsed, float delta)
    {
        elapsed += delta;
        if (elapsed < IntervalSeconds)
            return false;

        elapsed %= IntervalSeconds;
        return true;
    }
}
```

- [ ] **Step 5: Run the complete focused test executable and verify GREEN**

Run:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
```

Expected: exit code `0` and the exact final line `4 tests passed.`

- [ ] **Step 6: Build the plugin Release artifact**

Run:

```powershell
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
```

Expected: exit code `0`, zero errors, and `plugin/src/bin/Release/RevIdle.ScoreTelemetry.dll` exists.

- [ ] **Step 7: Write the concise build/install/configuration documentation**

Create `plugin/README.md`:

````markdown
# Revolution Idle Score Telemetry

This proof-of-concept BepInEx plugin sends the live Revolution Idle score to a local UDP listener every 50 ms.

Payload:

```json
{"score":"1.2345678901234567e123"}
```

## Requirements

- Revolution Idle for Windows x64 (Steam)
- BepInEx 6 IL2CPP build 785 or newer with generated `BepInEx/interop` assemblies
- .NET SDK 9 for building

The plugin itself targets .NET 6 because that is the runtime embedded by BepInEx IL2CPP.

## Build

From the repository root:

```powershell
dotnet run --project plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj
dotnet build plugin/src/RevolutionIdle.ScoreTelemetry.csproj -c Release
```

If the game is installed elsewhere, append `/p:GameDir='D:\path\to\Revolution Idle'` to both commands.

## Install

Create `BepInEx/plugins/RevIdle.ScoreTelemetry` under the Revolution Idle game folder and copy `plugin/src/bin/Release/RevIdle.ScoreTelemetry.dll` into it. Start the game once to create the config file:

`BepInEx/config/dev.tnthung.revolutionidle.scoretelemetry.cfg`

Configure the destination:

```ini
[Network]
Port = 19841
```

`0`, non-numeric values, and values outside `1..65535` disable output. The destination is always `127.0.0.1`; send failures are intentionally ignored.

## Receive a packet

Run this in PowerShell before starting the game:

```powershell
$udp = [Net.Sockets.UdpClient]::new(19841)
try {
    while ($true) {
        $packet = $udp.ReceiveAsync().GetAwaiter().GetResult()
        [Text.Encoding]::UTF8.GetString($packet.Buffer)
    }
} finally {
    $udp.Dispose()
}
```

## Uninstall

Remove `BepInEx/plugins/RevIdle.ScoreTelemetry`. If no other plugins use BepInEx, its loader can also be removed by deleting the added `BepInEx`, `dotnet`, `winhttp.dll`, `doorstop_config.ini`, `.doorstop_version`, and `changelog.txt` entries beside `Revolution Idle.exe`. This plugin does not edit save files.
````

- [ ] **Step 8: Deploy and live-verify one localhost packet**

Copy only the built plugin DLL into:

```text
C:\Program Files (x86)\Steam\steamapps\common\Revolution Idle\BepInEx\plugins\RevIdle.ScoreTelemetry\RevIdle.ScoreTelemetry.dll
```

Bind a one-shot UDP listener to `127.0.0.1:19841`, launch Revolution Idle, and wait up to 30 seconds. Verification succeeds only when all of these are true:

1. `BepInEx/LogOutput.log` reports one discovered/loaded plugin and chainloader startup completion without a plugin exception.
2. The receiver gets a UTF-8 datagram from loopback.
3. The datagram parses as JSON and has exactly one string property named `score`.
4. The `score` value matches `^-?[0-9]+(?:\.[0-9]+)?(?:E[+-]?[0-9]+)?e-?[0-9]+(?:\.[0-9]+)?(?:E[+-]?[0-9]+)?$`, proving it came from the invariant round-trip mantissa/exponent path.

Close only the Revolution Idle process started for verification after capturing the evidence.

- [ ] **Step 9: Commit Task 2**

```powershell
git add plugin/src/RevolutionIdle.ScoreTelemetry.csproj plugin/src/Plugin.cs plugin/tests/RevIdle.ScoreTelemetry.Tests.csproj plugin/tests/Program.cs plugin/README.md
git commit -m "feat: publish live score from Revolution Idle"
```
