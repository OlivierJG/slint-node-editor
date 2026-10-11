# bummer.md — what did not work here and why.
# Append-only during tasks; the `bummer` skill merges, promotes and retires.
# Template per entry:
#
# ## YYYY-MM-DD · title · #tags/paths
# Situation: task, constraints, versions
# Tried: what seemed reasonable / what was proposed
# Outcome: what failed or was insufficient (observed, not inferred)
# Evidence: dated observation or user correction, reference if any
# Hypothesis: suspected cause (optional; keep separate from Outcome)
# Next time: verified alternative, or suggestion — say which
# Revisit when: conditions that would invalidate this
# Cost: what the dead end cost (time, reverts, user corrections)
# Scope: project | global-candidate     Status: active
# recurred: YYYY-MM-DD note   (append to the existing entry, no new entry)

## 2026-09-17 · Preview does not verify attachment availability · #README.md #docs/media #github #release
Situation: Publishing slint-node-editor 1.0.0 with a video in the README.
Tried: Used a GitHub user-attachment URL after checking the inline Markdown preview.
Outcome: The user reported a 404 after publication; an unauthenticated download confirmed it. The repository MP4 still returned HTTP 200.
Evidence: 2026-09-17 user report and HTTP checks for attachment 0c3e6c2f-ce5d-4dfb-b2df-67c7bdc611fc and the tagged repository MP4.
Next time: Verify media URLs without authentication before publication. A versioned GitHub release asset is an alternative for the download link; verify its contents against the committed MP4.
Revisit when: A replacement attachment is publicly accessible and its persistence has been verified independently of an editor preview.
Cost: User correction and post-release documentation repair; the published 1.0.0 crate still contains the original README.
Scope: project     Status: active
recurred: 2026-09-17 The user reported the crates.io link was still broken after the repository README fix. The crates.io 1.0.0 README endpoint confirmed the original attachment URL remained embedded. Repairing main alone was insufficient; publish a documentation patch and verify the README served by crates.io.

## 2026-09-18 · Fixed window dimensions invalidate native resize probes · #integration-tests #slint #viewport
Situation: Reproducing issue #6 with Slint 1.18.0 and the headless integration-test window.
Tried: Dispatched a native resize event and then used Window::set_size while the fixture declared width: 800px and height: 600px.
Outcome: The editor stayed 800 by 600 and no resize notification fired. Generated bindings for the window's exposed dimensions and editor size were constants, so the test was not exercising a resizable viewport.
Evidence: The failing viewport_resize test printed unchanged window/editor sizes; generated test.rs initialized those properties with literal 800 and 600 values.
Next time: Verified alternative: use preferred-width/preferred-height in a resizable fixture, resize through Window::set_size, and assert the actual dimensions before checking the callback result.
Revisit when: Testing a deliberately fixed-size window or a Slint version with different sizing semantics.
Cost: Two additional failing test runs and generated-code inspection.
Scope: project     Status: active

## 2026-09-18 · Checking only the last callback misses notification semantics · #viewport #integration-tests #api
Situation: PR #8 introduced viewport-resized for hosts maintaining their visible bounds.
Tried: Asserted the last callback dimensions after resizing and documented connecting a handler, without checking the number of notifications or initial state.
Outcome: The Claude review and a runtime probe found two identical callbacks for a combined width/height resize and no initial notification, including for a handler connected before show().
Evidence: The probe recorded [(1000.0, 700.0), (1000.0, 700.0)] for one resize and [] on initial subscription.
Next time: Verify the full event sequence for batched changes and explicitly test/document how a new subscriber obtains its initial state. The fix coalesces size changes and documents host seeding.
Revisit when: An API explicitly requires per-property events or provides a replaying subscription mechanism.
Cost: Adversarial-review follow-up and additional regression coverage.
Scope: project     Status: active

## 2026-09-25 · Brace-matching patch on `TypeName {` also hits impl blocks · #integration-tests #tooling #python-patch
Situation: Adding a `group_id: 0` field to 26 `NodeData { … }` literals across the integration tests with a Python brace-matching script keyed on the substring `NodeData {`.
Tried: Matched every `NodeData {`, walked to the closing brace, and inserted the field before it when the block mentioned `selected`.
Outcome: `impl MovableNode for NodeData { … }` in the harness matched too; the script turned its closing `}` into `},\n    group_id: 0,\n}` and the next edit script aborted on the corrupted anchor. Literals were all patched correctly.
Evidence: harness.rs lines 42–45 after the run showed `    },` followed by `group_id: 0,` inside the impl; found by grepping for `}` + `,` preceding the inserted line.
Hypothesis: A struct literal and an impl/fn body share the `Name {` prefix; the `selected` heuristic matched the impl because it has a `fn selected` method.
Next time: Anchor literal patches on a preceding token that only literals have (`= NodeData {`, `vec![NodeData {`, `.map(|…| NodeData {`, `NodeData {` at line end after `(`), or require the block to contain `field:` pairs and no `fn `. Verify with a grep for the insertion signature in a non-literal context before compiling. Suggestion, not verified beyond this run.
Revisit when: The patch is done by a syntax-aware tool (rust-analyzer structured edit) instead of text matching.
Cost: One corrupted file, one aborted edit script, three extra tool calls.
Scope: global-candidate     Status: active

## 2026-10-10 · Check struct exhaustiveness before declaring field additions breaking · #release #api #src/layout.rs
Situation: Assessing whether current main can be released as 1.1 after adding rank alignment and link styling.
Tried: Classified both SugiyamaConfig.rank_alignment and generated LinkData fields as breaking public struct additions without inspecting the released declaration's attributes.
Outcome: SugiyamaConfig was already #[non_exhaustive] in v1.0.1; external exhaustive literals were already forbidden, so the rank_alignment addition does not break them. The LinkData concern remains separate.
Evidence: 2026-10-10 comparison of git show v1.0.1:src/layout.rs with current src/layout.rs; corrected the earlier assessment to the user.
Next time: Inspect the released type's attributes and external construction contract before classifying an added field. Assess generated types independently.
Revisit when: A different API change affects existing constructors, trait implementations, or field behavior rather than exhaustive construction.
Cost: Incorrect release-planning claim and a follow-up correction.
Scope: project     Status: active

## 2026-10-10 · Growing an overlay can cover example nodes · #examples/custom-shapes #ui #review
Situation: Adding palette and focus controls to the custom-shapes example.
Tried: Expanded the floating panel and moved it to the bottom-left without checking it against the initial node positions.
Outcome: Claude's source review identified an overlap with Node C: the panel's 20–330 by 454–584 screen rectangle intersected the node's 100–250 by 400–500 rectangle at the initial viewport. The replacement uses a separate toolbar and the editor's actual dimensions.
Evidence: Read-only Claude review in /tmp/slint-docs-claude-review.txt on 2026-10-10, item 1; rectangles follow directly from the UI and initial model coordinates, not a native screenshot.
Next time: Check initial control and node rectangles after changing overlay size or placement. A separate layout row is a verified structural way to keep controls outside the editor viewport.
Revisit when: The overlay deliberately occludes content or the example has a defined reserved area under every viewport transform.
Cost: One implemented and compiled panel layout replaced after review.
Scope: project     Status: active
