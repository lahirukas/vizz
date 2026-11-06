# Next steps

- [ ] **Beat benchmark**: score the tracker on Harmonix, Ballroom and SMC with standard metrics
      (F-measure, CMLt, AMLt), tune parameters, and publish the results.
- [ ] **Standalone beat engine**: split `crates/dsp` into its own repository and crate with a stable
      real-time API, examples and the benchmark scores.
- [ ] **VIZZ logo moments**: the sine-wave wordmark forms inside each design on detected drops and when
      music returns after silence (rate-limited, at most once every few minutes).
- [ ] **Signed releases**: get SignPath Foundation approval (see `docs/CODE_SIGNING.md`) so release
      builds pass SmartScreen without warnings.
