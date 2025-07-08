function safeCalls(obj: any) {
    // These should be matched and replaced
    obj.method && obj.method();
    obj.callback && obj.callback(data);

    // These should not be matched
    obj.method();
    obj?.method?.();
    if (obj.method) {
        obj.method();
    }
}
