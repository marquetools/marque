<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00525">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
        higher than the classification value.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule checks the value of @ism:classification.  If the value of @ism:highWaterNATO
        is 'NATO-TS' then @ism:classification must be 'TS'. If the value of @ism:highWaterNATO is 'NATO-S',
        then @ism:classification must be 'TS' or 'S'. If the value of @ism:highWaterNATO is 'NATO-C', then
        @ism:classification must be 'C', 'S' or 'TS'.  If the value of @ism:highWaterNATO is 'NATO-R' then
        @ism:classification cannot be 'U'.  If the value of @ism:highWaterNATO is 'NATO-U' then any value
        of classification is ok.  </sch:p>
    <sch:rule id="ISM-ID-00525-R1" context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]">
        <sch:assert
            test="if (normalize-space(string(./@ism:highWaterNATO))='NATO-TS' and normalize-space(string(./@ism:classification))='TS') 
               then true()
            else if (normalize-space(string(./@ism:highWaterNATO))='NATO-S' and (normalize-space(string(./@ism:classification))='S'
            or normalize-space(string(./@ism:classification))='TS')) 
               then true()
            else if (normalize-space(string(./@ism:highWaterNATO))='NATO-C' and (normalize-space(string(./@ism:classification))='TS' 
            or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C')) 
               then true()
            else if (normalize-space(string(./@ism:highWaterNATO))='NATO-R' and not(normalize-space(string(./@ism:classification))='U')) 
               then true()
            else if (normalize-space(string(./@ism:highWaterNATO))='NATO-U') then true()
            else false()" 
            flag="error" role="error"> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
            attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
            higher than the classification value. </sch:assert>
    </sch:rule>
</sch:pattern>
