<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00012">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText"> 
        [ISM-ID-00012][Error] If the document is not USA-CUI-ONLY, AND: 
        1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
        @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
        OR
        2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access or ntk:AccessProfile,
        then attributes @ism:classification and @ism:ownerProducer must be specified for the element.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If the document does NOT have @ism:compliesWith="USA-CUI-ONLY", then for
        each element which defines an attribute in the ISM namespace other than @ism:pocType,
        @ism:DESVersion, @ism:ISMCATCESVersion, or @ism:unregisteredNoticeType, or the element is arh:Security,
        or arh:ExternalSecurity or ntk:Access or ntk:AccessProfile, this rule ensures that
        attributes @ism:classification and @ism:ownerProducer are specified. </sch:p>
    <sch:rule id="ISM-ID-00012-R1"
        context="*[((@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion)) 
        or (self::arh:ExternalSecurity or self::ntk:Access or self::ntk:ExternalAccess or self::ntk:AccessProfile))
        and not($ISM_USCUIONLY_RESOURCE)]">
        <sch:assert test="@ism:ownerProducer and @ism:classification" flag="error" role="error">
            [ISM-ID-00012][Error] If the document does NOT have @ism:compliesWith="USA-CUI-ONLY",
            then if:
            1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
            @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
            OR 
            2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access, or ntk:AccessProfile,
            then attributes @ism:classification and @ism:ownerProducer must be specified for the element.
        </sch:assert>
    </sch:rule>
</sch:pattern>
