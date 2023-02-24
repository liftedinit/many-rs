load("@rules_pkg//pkg:providers.bzl", "PackageVariablesInfo")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@bazel_bats//:rules.bzl", "bats_test")

# Taken from Bazel repository
def _basic_naming_impl(ctx):
    values = {}

    # Copy attributes from the rule to the provider
    values["product_name"] = ctx.attr.product_name
    values["version"] = ctx.attr.version
    values["revision"] = ctx.attr.revision
    values["compilation_mode"] = ctx.var.get("COMPILATION_MODE")

    return PackageVariablesInfo(values = values)

### RUSTC BUILD ARG RULE ###
# A rule to inject variables from the build file into package names.

basic_naming = rule(
    implementation = _basic_naming_impl,
    attrs = {
        "product_name": attr.string(
            doc = "Placeholder for our final product name.",
        ),
        "revision": attr.string(
            doc = "Placeholder for our release revision.",
        ),
        "version": attr.string(
            doc = "Placeholder for our release version.",
        ),
    },
)

# Taken from https://github.com/bazelbuild/rules_rust/issues/801
def _rustc_flags_file_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    cfg_lines = [
        "--cfg\nfeature=\"%s\"" % flag.label.name
        for flag in ctx.attr.flags
        if flag[BuildSettingInfo].value
    ]
    ctx.actions.write(
        output = out,
        content = "\n".join(cfg_lines),
    )
    return [DefaultInfo(files = depset([out]))]

rustc_flags_file = rule(
    implementation = _rustc_flags_file_impl,
    attrs = {
        "flags": attr.label_list(),
    },
)
### END RUSTC BUILD ARG RULE ###

### RUN MAKE RULE ###
# Bazel rule to _run_ Makefile commands
#
# Note: This rule is not meant to be used in other projects

def _run_make(ctx):
    executable = ctx.actions.declare_file("run-make.sh")

    # TODO: This is dumb, fix
    cmds = ["cd docker && make -f %s %s" % (ctx.file.src.short_path, ctx.attr.cmd)]

    ctx.actions.write(
        content = "#!/bin/bash\n" + " && ".join(cmds),
        output = executable,
        is_executable = True,
    )

    return executable

def _run_make_impl(ctx):
    executable = _run_make(ctx)
    runfiles = ctx.runfiles(files = ctx.files.data)
    transitive_runfiles = []
    for runfiles_attr in (
        ctx.attr.data,
    ):
        for target in runfiles_attr:
            transitive_runfiles.append(target[DefaultInfo].default_runfiles)
    runfiles = runfiles.merge_all(transitive_runfiles)

    return DefaultInfo(
        executable = executable,
        runfiles = runfiles,
    )

run_make = rule(
    attrs = {
        "src": attr.label(
            allow_single_file = True,
            doc = "Makefile file name",
        ),
        "data": attr.label_list(
            allow_files = True,
            doc = "Data dependencies",
        ),
        "cmd": attr.string(
            doc = "Makefile command",
        ),
    },
    executable = True,
    implementation = _run_make_impl,
)

### END RUN MAKE RULE ###

### BATS TEST SUITE ###
# TODO: Remove when https://github.com/filmil/bazel-bats/pull/18 is merged and a new `bazel-bats` release is out
def bats_test_suite(name, srcs, **kwargs):
    tests = []

    for src in srcs:
        if not src.endswith(".bats"):
            fail("srcs should have `.bats` extensions")

        # Prefixed with `name` to allow parameterization with macros
        # The test name should not end with `.bats`
        test_name = name + "_" + src[:-5]
        bats_test(
            name = test_name,
            srcs = [src],
            **kwargs
        )
        tests.append(test_name)

    native.test_suite(
        name = name,
        tests = tests,
        tags = kwargs.get("tags", None),
    )

### END BATS TEST SUITE ###
