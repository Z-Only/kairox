export default {
  extends: ["stylelint-config-recommended-vue"],
  overrides: [
    {
      files: ["**/*.{vue,html}"],
      customSyntax: "postcss-html"
    }
  ],
  rules: {
    "no-descending-specificity": null,
    "selector-pseudo-class-no-unknown": [
      true,
      {
        ignorePseudoClasses: ["deep", "global", "slotted"]
      }
    ]
  }
};
