import { afterEach, beforeEach, vi } from "vitest";

export const imageUrlMocks = {
  createObjectURL: vi.fn<(blob: Blob) => string>(),
  revokeObjectURL: vi.fn<(url: string) => void>(),
};

/** Install per-test Blob URL spies without replacing the browser URL constructor. */
export function installImageUrlMocks() {
  let createDescriptor: PropertyDescriptor | undefined;
  let revokeDescriptor: PropertyDescriptor | undefined;
  let nextId = 0;

  beforeEach(() => {
    createDescriptor = Object.getOwnPropertyDescriptor(URL, "createObjectURL");
    revokeDescriptor = Object.getOwnPropertyDescriptor(URL, "revokeObjectURL");
    nextId = 0;
    imageUrlMocks.createObjectURL.mockReset();
    imageUrlMocks.revokeObjectURL.mockReset();
    imageUrlMocks.createObjectURL.mockImplementation(() => `blob:clipstash-test-${++nextId}`);
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      value: imageUrlMocks.createObjectURL,
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      configurable: true,
      value: imageUrlMocks.revokeObjectURL,
    });
  });

  afterEach(() => {
    if (createDescriptor) Object.defineProperty(URL, "createObjectURL", createDescriptor);
    else delete (URL as URL & { createObjectURL?: typeof URL.createObjectURL }).createObjectURL;
    if (revokeDescriptor) Object.defineProperty(URL, "revokeObjectURL", revokeDescriptor);
    else delete (URL as URL & { revokeObjectURL?: typeof URL.revokeObjectURL }).revokeObjectURL;
  });

  return imageUrlMocks;
}
