# Kith agent instructions

Kith is a runtime layer on [Uffda](https://github.com/justinmchase/uffda):
components, message dispatch, and workflows. Higher-level languages such as
[Coleslaw](https://github.com/justinmchase/coleslaw) are built on Kith; they
MUST NOT redefine these primitives.

## Specifications

This repository follows the specs method, as declared in `.agents/SPECS`
(<https://github.com/justinmchase/specs>).

- Authority, highest first: specifications (`.agents/specifications/`),
  requirements (`.agents/requirements/`), tests, implementation. Never change a
  lower layer to contradict a higher one; if a request conflicts with the
  specification or a requirement, ask before proceeding.
- A behavior change updates the specification or requirement that covers it in
  the same change as the code.
- Tests cite the requirements they verify with `req:{id}`.
- Before writing specifications or requirements, use the `specs` and
  `requirements` skills. Without the plugin installed, read them at
  <https://raw.githubusercontent.com/justinmchase/specs/v1/skills/specs/SKILL.md>
  and
  <https://raw.githubusercontent.com/justinmchase/specs/v1/skills/requirements/SKILL.md>.
