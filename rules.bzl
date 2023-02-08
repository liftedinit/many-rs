load("@rules_pkg//pkg:providers.bzl", "PackageVariablesInfo")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

# Taken from Bazel repository
def _basic_naming_impl(ctx):
    values = {}

    # Copy attributes from the rule to the provider
    values["product_name"] = ctx.attr.product_name
    values["version"] = ctx.attr.version
    values["revision"] = ctx.attr.revision
    values["compilation_mode"] = ctx.var.get("COMPILATION_MODE")

    return PackageVariablesInfo(values = values)

#
# A rule to inject variables from the build file into package names.
#
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
