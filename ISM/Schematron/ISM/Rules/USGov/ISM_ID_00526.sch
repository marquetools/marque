<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00526">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and the @ism:ownerProducer
        attribute contains multiple values on a banner or portion, one being NATO, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:ownerProducer attribute contains multiple values and NATO, then @ism:highWaterNATO must exist. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:ownerProducer attribute contains multiple values and NATO. </sch:p>
    <sch:rule id="ISM-ID-00526-R1" context="*[$ISM_NSI_EO_APPLIES and contains(@ism:ownerProducer,'NATO') and not(@ism:ownerProducer='NATO')]">
        <sch:assert
            test="@ism:highWaterNATO"
            flag="error" role="error"> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and @ism:ownerProducer
            attribute contains multiple values on a banner or portion, one being NATO, then @ism:highWaterNATO must exist.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
            element when @ism:ownerProducer attribute contains multiple values and NATO. </sch:assert>
    </sch:rule>
</sch:pattern>