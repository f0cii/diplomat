// generated by diplomat-tool
import type { Utf16Wrap } from "./Utf16Wrap"
import type { pointer, char } from "./diplomat-runtime.d.ts";

export class OpaqueMutexedString {
    

    get ffiValue(): pointer;


    static fromUsize(number: number): OpaqueMutexedString;

    change(number: number): void;

    borrow(): OpaqueMutexedString;

    static borrowOther(other: OpaqueMutexedString): OpaqueMutexedString;

    borrowSelfOrOther(other: OpaqueMutexedString): OpaqueMutexedString;

    getLenAndAdd(other: number): number;

    dummyStr(): string;

    wrapper(): Utf16Wrap;

    

}