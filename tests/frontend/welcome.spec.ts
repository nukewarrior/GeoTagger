import { createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import WelcomeView from "@/views/WelcomeView.vue";

describe("welcome view", () => {
  it("states the local-only safety promise and exposes the main entry points", async () => {
    const wrapper = mount(WelcomeView, {
      global: {
        plugins: [createPinia()],
      },
    });

    expect(wrapper.text()).toContain("照片、轨迹与坐标始终留在本机");
    expect(wrapper.text()).toContain("新建项目");
    expect(wrapper.text()).toContain("打开项目");
    expect(wrapper.text()).toContain("体验离线演示");
  });

  it("opens the new-project dialog without invoking desktop APIs", async () => {
    const wrapper = mount(WelcomeView, {
      global: {
        plugins: [createPinia()],
      },
    });

    await wrapper.get(".primary-button").trigger("click");

    expect(wrapper.text()).toContain("创建一个本地项目");
    expect(wrapper.text()).toContain("默认只读取原照片");
  });
});
