import { config } from "@vue/test-utils";

config.global.stubs = {
  transition: false,
  "transition-group": false,
};

class ResizeObserverStub implements ResizeObserver {
  disconnect(): void {}
  observe(): void {}
  unobserve(): void {}
}

globalThis.ResizeObserver = ResizeObserverStub;
