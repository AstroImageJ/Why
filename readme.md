# Why, a Java launcher made in Rust

A launcher in Rust to launch Java applications for Windows.

Features:
- Configurable JVM lookup
  - If not specified, will try the current working directory (depth of 4)
  - Has configurable fallback to `JAVA_HOME` and (TODO)common Java installation paths
- Java version validation
- Configuration done through `launcher.ini`

Drawbacks:
- Only works with `main` method type programs

# Why?
- Launch4j does not set process name, and seems to causes issues with Windows UI scaling
- winrun4j can't read Launch4j's config at usertime
- I just wanted something to work