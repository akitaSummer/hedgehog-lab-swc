const swc = require("../pkg/hedgehog_lab_swc");
const it = require('ava');

it("should be loadable", (t) => {
    const output = swc.transformSync(`
    if (foo) {
        console.log("Foo")
    } else {
        console.log("Bar")
    }`);
    t.is(output.code, `if (foo) {
    void 0("Foo");
} else {
    void 0("Bar");
}
`)
});
