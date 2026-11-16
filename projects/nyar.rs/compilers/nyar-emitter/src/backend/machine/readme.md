# Native image backend

This module currently classifies an already encoded native image, checks architecture and container consistency, and wraps it as an artifact. It does not itself establish machine-code generation, object lowering or linking support.

The native target architecture separates machine/object/link/container services from language-to-native adaptation and extension ABI layers. `.node` needs Node-API initialization and lifecycle. `.pyd` / Python `.so` needs CPython initialization, ownership and exception bridging. An accepted artifact extension alone proves none of these capabilities.
