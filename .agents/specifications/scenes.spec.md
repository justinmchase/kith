# Scenes

This chapter defines scenes: the design-time tree that instantiates component
kinds and wires parent/child slots. Component machines are defined in
[components](./components.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Scene declaration**: the source that names a scene and the instance tree it
  constructs.
- **Instance construction**: a node in that tree: a component kind, an optional
  slot key under its parent, and nested constructions for its children.
- **Slot key**: the name by which a parent reaches a direct child. It is the
  `key` field of that child's routing record.
- **Routing record**: the value `{ key, component }` for one direct child,
  used only to select send-down targets. It MUST NOT include the child's
  fields or machine state.

## Axioms

- A running program is the tree a scene built. Kinds do not own kinds.
- An instance other than the root has exactly one parent.
- The tree is has-a: no instance has two parents, and no instance is its own
  ancestor.

## Constraints

- A scene MUST have exactly one top-level instance. That instance is the
  program root when the scene is run.
- Nested construction `key = Kind() { ... }` MUST fill the slot `key` under
  the surrounding instance with one instance of `Kind`.
- A scene MUST NOT share an instance between two parents.
- Check MUST fail if a construction names an unknown kind.
- Check MUST fail if two children under one parent use the same slot key.
- Components MUST NOT declare children. Children exist only in a scene.
- `kith run` and `World::start` MUST run the unique exported scene of the
  program. A program that exports no scene, or more than one scene, MUST be
  rejected.

## Mechanism

### Declaration

```kith
export scene Main {
  Parent() {
    child = Child()
  }
}
```

- `scene Name { ... }` declares a named scene.
- `Kind()` constructs an instance of `Kind`. The top-level construction has no
  slot key.
- `key = Kind() { ... }` constructs a child in slot `key`. Nested braces are
  further children of that instance.
- `export scene Name` or `export Name;` (when `Name` is a scene) makes the
  scene the program's run target when it is the unique exported scene.

The normative ping program is in [components](./components.spec.md#declaration).
Its scene is `Main`: a `Parent` root with one child in slot `child` of kind
`Child`.

### Import and export

- `export Name Name*;` and `export` on a declaration MUST use Uffda's export
  list shape. Exported names are the components and scenes other modules, and
  `kith run`, MAY take.
- `import "specifier" Name Name*;` MUST use Uffda's import list shape.
- Relative `.kith` specifiers MUST load that file and take the named exports
  from it. The imported components and scenes MUST have been exported by that
  file. Package and `jsr:` specifiers are not required of this slice.

### Routing records

When an instance sends down, each direct child is offered as:

```text
{ key: "<slot key>", component: "<kind name>" }
```

`send down` is defined in [components](./components.spec.md#sending).

## Why this design

- **Scene versus prefab.** Declaring `child Child child` on Parent couples a
  reusable kind to a particular graph. A scene is the Unity-style place that
  builds the tree; a component is only a machine and fields.
- **Keys, not injected handles.** Slot names live on the routing record so
  `send down { key: "child" }` selects a role without putting scene bindings
  in the component's variable scope.
- **Tree, not DAG.** Sharing an instance would give it two parents and break
  send-up. Nested construction is enough; flat bind-then-inject waits.
