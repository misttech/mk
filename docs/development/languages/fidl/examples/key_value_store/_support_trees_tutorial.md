In this variant, we allow our key-value store to take other key-value stores as
members. In short, we turn it into a tree. We do this by replacing the original
definition of `value` with one that utilizes a two-member `union`: one variant
stores leaf nodes using the same `vector<byte>` type as before, while the other
stores branch nodes in the form of other nested stores.

### Reasoning {:.hide-from-toc}

Here, we see several uses of *optionality*, whereby we can declare a type that
may or may not exist. There are three flavors of optionality in FIDL:

- Types that have are always stored
  [out-of-line](/docs/reference/fidl/language/wire-format#primary_and_secondary_objects)
  on the wire, and thus have a builtin way to describe "absentness" via the
  [null envelope](/docs/reference/fidl/language/wire-format#envelopes). Enabling
  optionality for these types doesn't affect the wire shape of messages they are
  included in - it simply changes which values are valid for that particular
  type. The `union`, `vector<T>`, `client_end`, `server_end`, and `zx.Handle`
  types can all be made optional via the addition of the `:optional` constraint.
  By making our `value` `union` optional, we are able to introduce a canonical
  "null" entry, in the form of an absent `value`. This means that empty `bytes`
  and absent/empty `store` properties are invalid values.
- Unlike the aforementioned types, the `struct` layout has no extra space where
  a null header can be stored. Because of this, it needs to be wrapped in an
  envelope, changing the on-the-wire shape of the message it is being included
  in. To ensure that this wire-modifying effect easily legible, the `Item`
  `struct` type must be wrapped in a `box<T>` type template.
- Finally, `table` layouts are always optional. An absent `table` is simply one
  with none of its members set.

Trees are a naturally self-referential data structure: any node in the tree may
contain a leaf with pure data (in this case, a string), or a sub-tree with more
nodes. This requires recursion: the definition of `Item` is now transitively
dependent on itself! Representing *recursive types* in FIDL can be a bit tricky,
especially because support is currently [somewhat
limited](https://fxbug.dev/42110612). We can support such types as long as there is
at least one optional type in the cycle created by the self-reference. For
instance, here we define the `items` `struct` member to be a `box<Item>`,
thereby breaking the includes cycle.

<!-- TODO(https://fxbug.dev/42110612): remove box<Item> once this lands -->

These changes also make heavy use of *anonymous types*, or types whose
declarations are inlined at their sole point of use, rather than being named,
top-level `type` declarations of their own. By default, the names of anonymous
types in the generated language bindings are taken from their local context. For
instance, the newly introduced `flexible union` takes on its owning member's
name `Value`, the newly introduced `struct` would become `Store`, and so on.
Because this heuristic can sometimes cause collisions, FIDL provides an escape
hatch by allowing the author to manually override an anonymous type's *generated
name*. This is done via the `@generated_name` attribute, which allows one to
change the name generated by backends. We can use one here, where the would-be
`Store` type is renamed to `NestedStore` to prevent a name collision with the
`protocol` declaration that uses that same name.

### Implementation {:.hide-from-toc}

Note: The source code for this example is located at
[//examples/fidl/new/key_value_store/support_trees](/examples/fidl/new/key_value_store/support_trees).
This directory includes tests exercising the implementation in all supported
languages, which may be run locally by executing the following from
the command line: `fx set core.x64 --with=//examples/fidl/new:tests && fx test
keyvaluestore_supporttrees`.

The FIDL, CML, and realm interface definitions are modified as follows:

<div>
  <devsite-selector>
    <!-- FIDL -->
    <section>
      <h3 id="key_value_store-support_trees-fidl">FIDL</h3>
      <pre class="prettyprint">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/fidl/key_value_store.test.fidl" highlight="diff_1" %}</pre>
    </section>
    <!-- CML -->
    <section style="padding: 0px;">
      <h3>CML</h3>
      <devsite-selector style="margin: 0px; padding: 0px;">
        <section>
          <h3 id="key_value_store-support_trees-cml-client">Client</h3>
          <pre class="prettyprint">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/meta/client.cml" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-server">Server</h3>
          <pre class="prettyprint">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/meta/server.cml" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-realm">Realm</h3>
          <pre class="prettyprint">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/realm/meta/realm.cml" %}</pre>
        </section>
      </devsite-selector>
    </section>
  </devsite-selector>
</div>

Client and server implementations can then be written in any supported language:

<div>
  <devsite-selector>
    <!-- Rust -->
    <section style="padding: 0px;">
      <h3>Rust</h3>
      <devsite-selector style="margin: 0px; padding: 0px;">
        <section>
          <h3 id="key_value_store-support_trees-rust-client">Client</h3>
          <pre class="prettyprint lang-rust">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/rust/client/src/main.rs" highlight="diff_1,diff_2,diff_3" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-rust-server">Server</h3>
          <pre class="prettyprint lang-rust">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/rust/server/src/main.rs" highlight="diff_1,diff_2,diff_3,diff_4,diff_5" %}</pre>
        </section>
      </devsite-selector>
    </section>
    <!-- C++ (Natural) -->
    <section style="padding: 0px;">
      <h3>C++ (Natural)</h3>
      <devsite-selector style="margin: 0px; padding: 0px;">
        <section>
          <h3 id="key_value_store-support_trees-cpp_natural-client">Client</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/cpp_natural/TODO.md" region_tag="todo" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-cpp_natural-server">Server</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/cpp_natural/TODO.md" region_tag="todo" %}</pre>
        </section>
      </devsite-selector>
    </section>
    <!-- C++ (Wire) -->
    <section style="padding: 0px;">
      <h3>C++ (Wire)</h3>
      <devsite-selector style="margin: 0px; padding: 0px;">
        <section>
          <h3 id="key_value_store-support_trees-cpp_wire-client">Client</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/cpp_wire/TODO.md" region_tag="todo" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-cpp_wire-server">Server</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/cpp_wire/TODO.md" region_tag="todo" %}</pre>
        </section>
      </devsite-selector>
    </section>
    <!-- HLCPP -->
    <section style="padding: 0px;">
      <h3 id="key_value_store-support_trees-hlcpp">HLCPP</h3>
      <devsite-selector style="margin: 0px; padding: 0px;">
        <section>
          <h3 id="key_value_store-support_trees-hlcpp-client">Client</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/hlcpp/TODO.md" region_tag="todo" %}</pre>
        </section>
        <section>
          <h3 id="key_value_store-support_trees-hlcpp-server">Server</h3>
          <pre class="prettyprint lang-cc">{% includecode gerrit_repo="fuchsia/fuchsia" gerrit_path="examples/fidl/new/key_value_store/support_trees/hlcpp/TODO.md" region_tag="todo" %}</pre>
        </section>
      </devsite-selector>
    </section>
  </devsite-selector>
</div>
