# Scenes

This chapter defines scenes: the design-time tree that instantiates component
kinds, fills declared ports, and may add unnamed children. Component machines
and ports are defined in [components](./components.spec.md). Terms are defined
in the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Scene declaration**: the source that names a scene and the instance tree it
  constructs.
- **Instance construction**: a node in that tree: a component kind, an optional
  port name under its parent, and nested constructions for its children.
- **Port fill**: `name = Kind()` under a parent, assigning that instance to the
  parent's declared port `name`.
- **Unnamed child**: `Kind()` under a parent with no port name. It belongs to
  the parent's child collection and is reached only by kind.

## Axioms

- A running program is the tree a scene built. Kinds do not own the graph.
- An instance other than the root has exactly one parent.
- The tree is has-a: no instance has two parents, and no instance is its own
  ancestor.
- Named ports and unnamed children are one collection of direct children.

## Constraints

- A scene MUST have exactly one top-level instance. That instance MUST be a
  workflow. It is the program root when the scene is run.
- `name = Kind() { ... }` MUST fill port `name` with one instance of `Kind`.
- `Kind() { ... }` with no `name =` MUST add an unnamed child of that kind.
- A scene MUST NOT share an instance between two parents.
- Check MUST fail if a construction names an unknown kind.
- Check MUST fail if two children under one parent use the same port name.
- Check MUST fail if a parent instance omits a declared port.
- Check MUST fail if `name = Kind()` does not match the port's declared kind
  (`A a` filled with `a = B()`).
- Check MUST fail if `name = Kind()` names a port the parent kind does not
  declare.
- `kith run` and `World::start` MUST run the unique exported scene of the
  program. A program that exports no scene, or more than one scene, MUST be
  rejected. Check MUST fail if the scene root is not a workflow.

## Mechanism

### Declaration

```kith
export scene Main {
  Ping() {
    child = Child();
    Logger();
  }
}
```

- `scene Name { ... }` declares a named scene.
- `Kind()` constructs an instance of `Kind`. The top-level construction has no
  port name.
- `name = Kind() { ... }` fills port `name`. Nested braces are further children
  of that instance.
- Bare `Kind() { ... }` is an unnamed child of the surrounding instance.
- `export scene Name` or `export Name;` (when `Name` is a scene) makes the
  scene the program's run target when it is the unique exported scene.

The normative ping program is in [workflows](./workflows.spec.md#declaration).
Its scene is `Main`: a `Ping` workflow root. Scene children under a workflow
are prototypes cloned when a task or map names that kind.

### Import and export

- `export Name Name*;` and `export` on a declaration MUST use Uffda's export
  list shape. Exported names are the components and scenes other modules, and
  `kith run`, MAY take.
- `import "specifier" Name Name*;` MUST use Uffda's import list shape.
- Relative `.kith` specifiers MUST load that file and take the named exports
  from it. The imported components and scenes MUST have been exported by that
  file. Package and `jsr:` specifiers are not required of this slice.

## Why this design

- **Ports versus the graph.** `Child child;` on a component is a Unity-style
  serialized field: a typed collaborator the scene must fill. Unnamed
  `Logger()` children are extra hierarchy, reached by kind, not extra ports.
- **Scene still builds the tree.** The kind names the ports; the scene says
  which instances exist and how they nest. Sharing an instance would give it
  two parents and break send-up.
