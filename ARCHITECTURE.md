# ZenSignal Architecture

```mermaid
graph TB
    subgraph "UI Layer (Iced)"
        UI[ZenSignal App]
        Sidebar[Sidebar Component]
        Charts[Chart Components]
        UI --> Sidebar
        UI --> Charts
    end

    subgraph "Connection Management"
        Scanner[Device Scanner<br/>btleplug]
        ConnThread[Connection Thread<br/>Tokio Runtime]
        StopFlag[AtomicBool<br/>Stop Flag]
    end

    subgraph "Data Collection"
        Arctic[Arctic Library<br/>Polar BLE Protocol]
        Handler[Event Handler]
        DataCollection[Data Collection Loop]
        Arctic --> Handler
        Handler --> DataCollection
    end

    subgraph "Data Processing"
        Channels[Channel Processors]
        TimeSeries[TimeSeries Storage]
        Channels --> TimeSeries
    end

    subgraph "Communication Channels"
        UpdateChannel[mpsc::Sender/Receiver<br/>SensorUpdate]
        CommandChannel[mpsc::Sender/Receiver<br/>ConnectionCommand]
    end

    %% User interactions
    User([User]) -->|Scan Devices| Scanner
    User -->|Select & Connect| UI
    User -->|Disconnect| UI

    %% Scanning flow
    Scanner -->|Discovered Devices| UI
    Sidebar -->|Display Devices| User

    %% Connection flow
    UI -->|Connect Command| CommandChannel
    CommandChannel -->|Device ID| ConnThread
    ConnThread -->|Initialize| Arctic
    ConnThread -->|Control| StopFlag
    
    %% Disconnect flow
    UI -->|Disconnect Command| CommandChannel
    CommandChannel -->|Stop Signal| StopFlag
    StopFlag -->|Terminate| DataCollection

    %% Data flow
    Arctic -->|Heart Rate<br/>ECG<br/>Accelerometer| Handler
    Handler -->|SensorUpdate| UpdateChannel
    UpdateChannel -->|Process Data| Channels
    Channels -->|Store| TimeSeries
    TimeSeries -->|Render| Charts
    Charts -->|Display| User

    %% Status feedback
    DataCollection -->|Connection Status| UpdateChannel
    UpdateChannel -->|Update State| UI

    style UI fill:#4a90e2
    style Arctic fill:#e27d60
    style Channels fill:#85dcb0
    style Scanner fill:#e8a87c
    style ConnThread fill:#c38d9e
```

## Component Descriptions

### UI Layer
- **ZenSignal App**: Main application state and message handler
- **Sidebar**: Device list, scan button, connect/disconnect controls
- **Charts**: Real-time visualization using plotters-iced (ECG, HR, RR, Accelerometer)

### Connection Management
- **Device Scanner**: Uses btleplug to discover nearby Polar devices
- **Connection Thread**: Manages async Polar sensor connections via Tokio runtime
- **Stop Flag**: Thread-safe atomic flag for graceful disconnection

### Data Collection
- **Arctic Library**: Handles Polar H10 Bluetooth protocol communication
- **Event Handler**: Receives sensor events and forwards to UI
- **Data Collection Loop**: Runs event loop with cancellation support via tokio::select!

### Data Processing
- **Channel Processors**: Separate processors for each data type (HR, RR, ECG, ACC)
- **TimeSeries Storage**: Efficient storage for streaming data points

### Communication
- **Update Channel**: Sends sensor data and connection status from backend to UI
- **Command Channel**: Sends connect/disconnect commands from UI to backend

## Data Flow

1. **Discovery**: User scans → btleplug discovers devices → UI displays list
2. **Connection**: User selects device → Command sent → Arctic connects → Status feedback
3. **Streaming**: Arctic receives data → Handler processes → Channels update → Charts render
4. **Disconnection**: User clicks disconnect → Stop flag set → Loop exits → State reset

## Thread Model

- **Main Thread**: Iced UI rendering and event handling (16ms tick rate)
- **Connection Thread**: Manages Polar device lifecycle and async operations
- **Data Thread**: Implicit in Arctic's event loop for BLE communication

## Key Design Decisions

- **Channel-based Communication**: Decouples UI from backend for responsiveness
- **Atomic Stop Flag**: Enables clean shutdown without blocking
- **tokio::sync::RwLock**: Allows Send-safe sharing across async tasks
- **Message Processing Loop**: Drains entire queue each tick to prevent lag
