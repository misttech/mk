# Testing Scenic and Escher

## Testability

Information about testability:

* All changes within Fuchsia need to adhere to the [Testability rubric](/docs/development/testing/testability_rubric.md).
* See also: [Test environments](/docs/contribute/testing/environments.md)

## Scenic test packages

You can specify packages in these ways:

* Everything:

  ```
  --with //bundles/tests
  ```

* Individually, if you want to build less packages:

  ```
  --with //src/ui:tests
  --with //src/ui/lib/escher:escher_tests
  ```

## Scenic and CQ

Tests are automatically run before submission by trybots for every change in
Fuchsia. See the `fuchsia-x64-release` and `fuchsia-x64-asan` bots on
[https://ci.chromium.org/p/fuchsia/builders](https://ci.chromium.org/p/fuchsia/builders).

### Add a new test suite to existing CQ tests

Most of the Scenic and Escher tests are written in
[Googletest](https://github.com/google/googletest/).
If you need to add test suites or test cases, you need to create test source files and add
them as a source of the test executables in corresponding `BUILD.gn` files.

### Adding a test package to CQ

To add a test package to CQ, you need to:

* Create test components;
* Reference the test package;
* Specify test environments.

#### Create test components {#create-test-components}

To add a new test package to CQ, you need to create a test component including source files,
metadata and BUILD.gn file. See
[Test component](/docs/development/testing/components/test_component.md).
You can include multiple test executables in one single test package, and all of them will be
executed if you run `fx test <test_package>` on host.

Examples of test packages:

- **allocation_unittests:** [//src/ui/scenic:allocation_unittests](/src/ui/scenic/BUILD.gn)
- **escher_tests:** [//src/ui/lib/escher:escher_tests](/src/ui/lib/escher/BUILD.gn)

#### Reference the test package {#reference-test-package}

To ensure the test is run on CQ, it requires an unbroken chain of dependencies that roll up to your
`fx set` command's available packages (expandable using the `--with` flag), typically
going through the all target of `//bundles/packages/tests:all`.

You need to make sure that there is a dependency chain from `//bundles/packages/tests:all` to
your test package. For more information, see
[Testing FAQ documentation](/docs/development/testing/faq.md#q_what-ensures-it-is-run).

#### Specify test environments {#specify-test-environments}

To ensure that the test is run on CQ, you also need to specify a
[test environment](/docs/contribute/testing/environments.md)
for each test executable in the package inside the test's `BUILD.gn` file.

Generally the environment is set to `environments = basic_envs`.
This specifies the test should be run on both QEMU (for arm64), FEMU and NUC (for x64), and using
both debug and release builds. For running on other environments, refer to
[Test environments](/docs/contribute/testing/environments.md).

Reference the test package transitively. For example, the packages above are
referenced by `//bundles/packages/tests:all` through `//bundles/packages/tests:scenic`.

## Unit tests and integration tests

To run tests locally during development:

### Running on device/emulator

Some of these tests require the test Scenic to connect to the real display controller.

Run `fx shell killall scenic.cm` to kill an active instance of Scenic.

* Run all Scenic tests:

  From host workstation, ensure `fx serve` is running, then:

  ```
    fx test \
      escher_tests \
      allocation_unittests \
      display_unittests \
      flatland_unittests \
      flatland_buffers_unittests \
      flatland_display_compositor_pixeltests \
      flatland_display_compositor_pixeltests_with_fake_display \
      flatland_engine_unittests \
      flatland_renderer_unittests \
      input_unittests \
      scenic_unittests \
      scheduling_unittests
  ```

  Or from Fuchsia target device:

  ```
  runtests --names escher_unittests,input_unittests,a11y_manager_apptests
  ```

* Run a specific test binary:

  From host workstation, ensure `fx serve` is running, then use the following
  command to run the `escher_unittests` test component:

  ```
  fx test escher_unittests
  ```

* Run a single test within a component:

  From host workstation, ensure `fx serve` is running, then:

  ```
  fx test escher_unittests -- --gtest_filter=NoOpMacTest.DummyTestCase
  ```

  See more documentation about the [glob pattern for the filter arg](https://github.com/google/googletest/blob/HEAD/docs/advanced.md).

* Run a specific component

  From your host workstation:

  ```
  fx test fuchsia-pkg://fuchsia.com/escher_unittests#meta/escher_unittests.cm
  ```

  Note: `escher_unittests.cm` can be swapped for [any test component](/src/ui/scenic/BUILD.gn) . There is also fuzzy matching!

### Host tests

  * `fx test --host` runs all host tests, but you probably only want to run Escher tests.
  *  Escher: To run `escher_unittests` locally on Linux: follow instructions in
     [Escher documentation](/docs/development/graphics/escher/concepts/building.md).

## Manual UI tests

After making big Scenic changes, you may want to do manual UI testing by running some example
applications to see if there is any regression.

### UI Examples

There are some examples available:

* **flatland-rainbow**
  - Basic example of creating a UI using Flatland commands, and connecting it to
    the scene graph using one of two approaches:
    - connecting to `fuchsia.element.GraphicalPresenter`
    - (legacy) serving `fuchsia.ui.app.ViewProvider`
  - **Source:** [`//src/ui/examples/flatland-rainbow`](/src/ui/examples/flatland-rainbow)
  - **Build dependency:** `//src/ui/examples/flatland-rainbow`
  - **Package URIs:**
    - `fuchsia-pkg://fuchsia.com/flatland-examples#meta/flatland-rainbow.cm`
    - `fuchsia-pkg://fuchsia.com/flatland-examples#meta/flatland-rainbow-vulkan.cm`

To run these applications, you need to include the following dependency in your `fx set`
configuration:

```shell
fx set workbench_eng.x64 --with "//src/ui/examples"
```

### Running UI examples

#### Running in session

You can launch the stories (modules) in any session you are in, via
`ffx session add <component_name>`.  From your host workstation, run:

  ```shell
  fx session add "fuchsia-pkg://fuchsia.com/flatland_examples#meta/flatland-rainbow.cm"
  ```

  to present the `View` created in the `flatland-rainbow` component.

<!-- Reference links -->

