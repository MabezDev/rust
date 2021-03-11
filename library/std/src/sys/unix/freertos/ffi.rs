use crate::ptr;

pub type UBaseType_t = libc::c_uint;
pub type BaseType_t = libc::c_int;
pub type TickType_t = u32;
pub type TaskHandle_t = *mut libc::c_void;
pub type QueueHandle_t = *mut libc::c_void;
pub type SemaphoreHandle_t = QueueHandle_t;
pub type TaskFunction_t = extern "C" fn(*mut libc::c_void) -> *mut libc::c_void;

pub const queueQUEUE_TYPE_MUTEX: u8 = 1;
pub const queueQUEUE_TYPE_RECURSIVE_MUTEX: u8 = 4;
pub const pdFALSE: BaseType_t = 0;
pub const pdTRUE: BaseType_t = 1;
pub const semGIVE_BLOCK_TIME: TickType_t = 0;
pub const queueSEND_TO_BACK: BaseType_t = 0;
pub const errCOULD_NOT_ALLOCATE_REQUIRED_MEMORY: BaseType_t = -1;

extern "C" {
    pub fn vTaskSuspend(handle: TaskHandle_t);
    pub fn vTaskDelete(handle: TaskHandle_t);
    pub fn xTaskGetCurrentTaskHandle() -> TaskHandle_t;
    pub fn vTaskDelay(xTicksToDelay: TickType_t);
    pub fn xQueueCreateMutex(ucQueueType: u8) -> QueueHandle_t;
    #[link_name = "xQueueTakeMutexRecursive"]
    pub fn xSemaphoreTakeRecursive(sem: SemaphoreHandle_t, tmo: TickType_t) -> BaseType_t;
    #[link_name = "xQueueGiveMutexRecursive"]
    pub fn xSemaphoreGiveRecursive(sem: SemaphoreHandle_t) -> BaseType_t;
    pub fn xQueueGenericReceive(
        xQueue: QueueHandle_t,
        pvBuffer: *const libc::c_void,
        xTicksToWait: TickType_t,
        xJustPeeking: BaseType_t,
    ) -> BaseType_t;
    pub fn xQueueGenericSend(
        xQueue: QueueHandle_t,
        pvItemToQueue: *const libc::c_void,
        xTicksToWait: TickType_t,
        xCopyPosition: BaseType_t,
    ) -> BaseType_t;
    #[link_name = "vQueueDelete"]
    pub fn vSemaphoreDelete(sem: SemaphoreHandle_t);
    #[link_name = "xQueueCreateCountingSemaphore"]
    pub fn xSemaphoreCreateCounting(max: UBaseType_t, initial: UBaseType_t) -> SemaphoreHandle_t;
    pub fn xPortGetTickRateHz() -> u32;
    #[cfg(target_device = "esp8266")]
    pub fn xTaskCreate(
        pxTaskCode: TaskFunction_t,
        pcName: *const libc::c_char,
        usStackDepth: u32,
        pvParameters: *const libc::c_void,
        uxPriority: UBaseType_t,
        pxCreatedTask: *mut TaskHandle_t,
    ) -> BaseType_t;
    #[cfg(target_device = "esp32")]
    pub fn xTaskCreatePinnedToCore(
        pxTaskCode: TaskFunction_t,
        pcName: *const libc::c_char,
        usStackDepth: u32,
        pvParameters: *const libc::c_void,
        uxPriority: UBaseType_t,
        pxCreatedTask: *mut TaskHandle_t,
        xCoreID: BaseType_t,
    ) -> BaseType_t;
    pub fn pvTaskGetThreadLocalStoragePointer(xTaskToQuery: TaskHandle_t, xIndex: BaseType_t) -> *mut libc::c_void;
    pub fn vTaskSetThreadLocalStoragePointer(xTaskToQuery: TaskHandle_t, xIndex: BaseType_t, pvValue: *mut libc::c_void);
}

#[cfg(target_device = "esp32")]
#[inline]
pub unsafe fn xTaskCreate(
    pxTaskCode: TaskFunction_t,
    pcName: *const libc::c_char,
    usStackDepth: u32,
    pvParameters: *const libc::c_void,
    uxPriority: UBaseType_t,
    pxCreatedTask: *mut TaskHandle_t,
) -> BaseType_t {
    const tskNO_AFFINITY: BaseType_t = BaseType_t::max_value();
    xTaskCreatePinnedToCore(pxTaskCode, pcName, usStackDepth, pvParameters, uxPriority, pxCreatedTask, tskNO_AFFINITY)
}

#[inline]
pub unsafe fn xSemaphoreCreateRecursiveMutex() -> SemaphoreHandle_t {
    xQueueCreateMutex(queueQUEUE_TYPE_RECURSIVE_MUTEX)
}

#[inline]
pub unsafe fn xSemaphoreCreateMutex() -> SemaphoreHandle_t {
    xQueueCreateMutex(queueQUEUE_TYPE_MUTEX)
}

#[inline]
pub unsafe fn xSemaphoreGive(sem: SemaphoreHandle_t) -> BaseType_t {
    xQueueGenericSend(sem, ptr::null(), semGIVE_BLOCK_TIME, queueSEND_TO_BACK)
}

#[inline]
pub unsafe fn xSemaphoreTake(sem: SemaphoreHandle_t, timeout: TickType_t) -> BaseType_t {
    xQueueGenericReceive(sem, ptr::null(), timeout, pdFALSE)
}
