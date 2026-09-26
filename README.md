# dejj

This is WIP dwarf-to-decomp-database tool, developed primarily for the *Breath of the Wild*
decompilation project, but aims to be flexible enough to work with matching decompilation projects
in general.

## Why a custom tool?

The `debug_info` in DWARF is a well-known standard, so any decompiler that parses DWARF
should be capable of importing type information. While this is true, there are some challenges
with the current tools:

1. Over-complex types. Types in the debug info are exactly laid out as-defined in the source,
   for example, if you have:
    ```cpp
    namespace sead {
    template <typename T>
    struct BaseVec2 {
        union {
            struct { T x; T y; };
            std::array<T, 2> e;
        }
    };

    template <typename T>
    class Policies {
    public:
        using Vec2Base = BaseVec2<T>;
    };

    template <typename T>
    struct Vector2: public Policies<T>::Vec2Base { /* ... */ }
    ```

   Even if the decompiler can handle anonymous types, it will still likely generate these unnecessary
   abstractions in between. What you almost certainly want as a representation for this in your decompile
   database is:
    ```cpp
    struct sead::Vector2f {
        float x;
        float y;
    }
    ```
   This tool achives this by implementing several type-optimization passes to eliminate CPP abstractions like these.

2. Inconsistent type names. `debug_info` is what it says it is - information for debug. It is not a
   source of truth for what the type is called. You would often have the same type named differently
   in different translation units, or somethings named with or without its containing namespace.
   This problem is compounded by type names appear in generics.
   While is fine for debugging tools to display what the type is, it's not ideal for generating a full picture
   of all the types in a decompilation project.

   This tool solves this because it knows the context of the decompilation project through the config file.
   For any ambiguous types in the `debug_info`, it uses `libclang` to parse the original source code AST
   to resolve the correct names.

3. Mapping to the original executable. For projects that do not aim to reconstruct the original executable,
   the DWARF compiled from the decomp project would not have the correct addresses if applied directly
   to the original executable. This means you would still likely need some other tool or custom script
   to fix up things.

   This tool is that "other tool" - it uses the config from the decomp project to import the things to the right places
   automatically.

## How to use?

This tool is still work-in-progress. Please stay tuned. You can watch the development progress by following
the *Breath of the Wild* decompilation project.
