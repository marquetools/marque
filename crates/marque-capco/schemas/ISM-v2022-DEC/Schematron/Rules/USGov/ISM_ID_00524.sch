<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00524">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:ownerProducer cannot be equal to 'NATO'.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator is not allowed
        where @ism:ownerProducer='NATO'. It is okay if @ism:ownerProducer contains 'NATO' and other
        tokens like 'USA'. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule produces an error if @ism:ownerProducer='NATO'. </sch:p>
    <sch:rule id="ISM-ID-00524-R1" context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]">
        <sch:assert
            test="not(./@ism:ownerProducer='NATO')"
            flag="error" role="error"> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:ownerProducer
            cannot be equal to 'NATO'. Human Readable: For documents under E.O. 13526, the NATO
            high-water indicator is not allowed where @ism:ownerProducer='NATO'. It is okay if
            @ism:ownerProducer contains 'NATO' and other tokens like 'USA'. </sch:assert>
    </sch:rule>
</sch:pattern>
