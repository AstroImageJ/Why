# JVM Selection

By default, Why makes a best-effort attempt to find a valid Java installation 
to use when starting the application. This is that process.

The supplied `jvm_path` may be an absolute path, 
allowing users to force a certain installation to be used.

Each path search checks the `min_java` version required.

3 entries are collected so that if one fails to start 
for any reason the others may be attempted.

```mermaid
graph
A[Launcher Start] -->B(Config is read)
    B --> C{Check main class}
    C -->|Enabled| D(Read Java version from main class)
    C -->|Disabled| E{JVM path specified}
    D --> E
    E -->|Yes| F(Search the specified directory)
    E -->|No| G(Search current directory)
    F --> H{System Java allowed}
    G --> H
    H -->|Yes| I(Search the Path)
    H -->|No| J{Java Lookup allowed}
    I --> J
    J -->|Yes| K(Search common install locations)
    J -->|No| L(Collect first 3 locations)
    K --> L    
```