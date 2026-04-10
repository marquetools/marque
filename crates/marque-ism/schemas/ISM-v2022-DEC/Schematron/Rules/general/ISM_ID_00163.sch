<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00163">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00163][Error] If attribute @ism:nonUSControls exists either 
        1. the attribute @ism:ownerProducer must equal [NATO] or a [NATO:NAC] 
            OR 
        2. the attribute @ism:FGIsourceOpen must contain [NATO] or a [NATO:NAC]
            OR
        3. the attribute @ism:FGIsourceProtected is used (This should only be the case when it is a resource level or super portion marking)
        
        Human Readable: NATO and NATO/NACs are the only owner of classification markings for which nonUSControls are currently authorized.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For each element which specifies attribute @ism:nonUSControls, this rule ensures that either the attributes 
        @ism:ownerProducer or @ism:FGIsourceOpen are specified with a value of [NATO] or [NATO:NAC]
        OR the @ism:FGIsourceProtected attribute is specified. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">        
        NOTE: The last case with @ism:FGIsourceProtected should only occur when the element is either a resource node or 
        a super-portion such as the marking of a table where the table contains one or more portions meeting 1 or 2 from the rule description 
        AND one or more portions with @ism:FGIsourceProtected is specified.
    </sch:p>
    <sch:rule id="ISM-ID-00163-R1" context="*[@ism:nonUSControls]">
        <sch:assert test="(matches(normalize-space(string(@ism:ownerProducer)), '^NATO:?') or matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO:?')) or @ism:FGIsourceProtected" flag="error" role="error">
            [ISM-ID-00163][Error] If attribute @ism:nonUSControls exists either 
            1. the attribute @ism:ownerProducer must equal [NATO] or a [NATO:NAC] 
            OR 
            2. the attribute @ism:FGIsourceOpen must contain [NATO] or a [NATO:NAC]
            OR
            3. the attribute @ism:FGIsourceProtected is used (This should only be the case when it is a resource level or super portion marking)
            
            Human Readable: NATO and NATO/NACs are the only owner of classification markings for which nonUSControls are currently authorized.
        </sch:assert>
    </sch:rule>
</sch:pattern>