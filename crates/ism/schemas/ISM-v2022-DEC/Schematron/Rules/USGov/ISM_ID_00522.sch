<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00522">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:FGIsourceOpen contains [NATO] or
        @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI]. Human
        Readable: For documents under E.O. 13526, the NATO high-water indicator can only exist on an
        element where either @ism:FGIsourceOpen contains [NATO] or @ism:ownerProducer contains
        [NATO] or @ism:FGIsourceProtected contains [FGI]. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule ensures that at least one of the attributes @ism:ownerProducer
        or @ism:FGIsourceOpen is specified with a value of [NATO] or @ism:FGIsourceProtected is
        specified with a value of [FGI]. </sch:p>
    <sch:rule id="ISM-ID-00522-R1" context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]">
        <sch:assert test="
                (matches(normalize-space(string(@ism:ownerProducer)), 'NATO') or
                matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO') or
                matches(normalize-space(string(@ism:FGIsourceProtected)), 'FGI'))"
            flag="error" role="error"> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:FGIsourceOpen
            contains [NATO] or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected
            contains [FGI]. Human Readable: For documents under E.O. 13526, the NATO high-water
            indicator can only exist on an element where either @ism:FGIsourceOpen contains [NATO]
            or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI].
        </sch:assert>
    </sch:rule>
</sch:pattern>
