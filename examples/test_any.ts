// These should be detected
let badVar: any = "oops";
let badArray: any[] = [];
function badFunc(param: any): void {}
const badArrow = (data: any) => data;

// These should not be detected
let goodVar: string = "good";
let goodArray: string[] = [];
function goodFunc(param: string): void {}
const goodArrow = (data: string) => data;
