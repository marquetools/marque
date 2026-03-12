<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00523">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and the @ism:FGIsourceOpen
        attribute contains [NATO] on a banner or portion, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:FGIsourceOpen contains [NATO], then @ism:highWaterNATO must exist. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:FGIsourceOpen contains [NATO]. </sch:p>
    <sch:rule id="ISM-ID-00523-R1" context="*[$ISM_NSI_EO_APPLIES and contains(@ism:FGIsourceOpen,'NATO')]">
        <sch:assert
            test="@ism:highWaterNATO"
                flag="error" role="error"> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and @ism:FGIsourceOpen
                contains [NATO] on a banner or portion, then @ism:highWaterNATO must be present.
                Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
                element when @ism:FGIsourceOpen contains [NATO] </sch:assert>
    </sch:rule>
</sch:pattern>