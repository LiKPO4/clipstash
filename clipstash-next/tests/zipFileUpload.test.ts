import { afterEach, expect, it, vi } from "vitest";
import { importZipFile } from "../src/zipFileUpload";
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
afterEach(() => invokeMock.mockReset());
function file(size: number) {
  const slice = vi.fn((start: number, end: number) => ({ arrayBuffer: async () => new ArrayBuffer(end-start) }));
  return { value: { name: "data.zip", size, slice, arrayBuffer: () => { throw new Error("whole-file read forbidden"); } } as unknown as File, slice };
}

it("sends bounded binary chunks serially with acknowledged offsets", async () => {
  const source=file(2*1048576+7);
  let firstDone!: (value:number)=>void;
  let appendCount=0;
  invokeMock.mockImplementation((command,args,options) => {
    if(command==="begin_zip_upload") return Promise.resolve("upload-a");
    if(command==="append_zip_upload") {
      expect(args).toBeInstanceOf(ArrayBuffer);
      expect(args.byteLength).toBeLessThanOrEqual(1048576);
      expect(options.headers["x-upload-id"]).toBe("upload-a");
      if(++appendCount===1) return new Promise<number>((resolve)=>{firstDone=resolve;});
      return Promise.resolve(Number(options.headers["x-upload-offset"])+args.byteLength);
    }
    if(command==="finish_zip_upload") return Promise.resolve({inserted_messages:1});
    throw new Error(`Unexpected ${command}`);
  });
  const result=importZipFile(source.value);
  await vi.waitFor(()=>expect(appendCount).toBe(1));
  expect(source.slice).toHaveBeenCalledTimes(1);
  firstDone(1048576);
  expect(await result).toEqual({inserted_messages:1});
  expect(source.slice.mock.calls).toEqual([[0,1048576],[1048576,2097152],[2097152,2097159]]);
  expect(invokeMock).toHaveBeenLastCalledWith("finish_zip_upload",{uploadId:"upload-a"});
});

it.each(["read","append","ack","finish"])("aborts the lease after %s failure",async(mode)=>{
  const source=file(5);
  if(mode==="read") source.slice.mockImplementation(()=>({arrayBuffer:async()=>{throw new Error("read failed");}}));
  invokeMock.mockImplementation((command)=>{
    if(command==="begin_zip_upload") return Promise.resolve("upload-b");
    if(command==="append_zip_upload") return mode==="append"?Promise.reject(new Error("write failed")):Promise.resolve(mode==="ack"?3:5);
    if(command==="finish_zip_upload") return Promise.reject(new Error("invalid zip"));
    if(command==="abort_zip_upload") return Promise.resolve();
    throw new Error(`Unexpected ${command}`);
  });
  await expect(importZipFile(source.value)).rejects.toThrow();
  expect(invokeMock).toHaveBeenLastCalledWith("abort_zip_upload",{uploadId:"upload-b"});
});
