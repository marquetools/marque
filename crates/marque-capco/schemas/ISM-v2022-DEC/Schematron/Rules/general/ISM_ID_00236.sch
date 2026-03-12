<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00236">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText"> 
        [ISM-ID-00236][Error] Duplicate tokens are not permitted in ISM attributes.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc"> 
        To determine the valid values, this rule first retrieves the CVE values for the attribute, 
        which in this case is classification. Then, each attribute token is converted into a numerical value
        based on its characters. Next, each attribute token is given an order number, which compares its position
        to that of its value in the CVE file. If the token is not found, its order number will be -1. 
        If the document is an IC resource and the ownerProducer of this element is 'USA', then the rule will fail
        if tokens are found with order numbers of -1. The rule will also fail if duplicate values are found for the element,
        or when its count is greater than 1. </sch:p>
    <sch:rule id="ISM-ID-00236-R1" context="*[@ism:*]">
        <!-- Determine if the list has duplicate values. If and only if it does, figure out which ones are duplicates -->
        <sch:let name="dupAttrs" value="for $attr in ./(@ism:atomicEnergyMarkings, @ism:classification, @ism:compliesWith, @ism:declassException, @ism:displayOnlyTo, @ism:disseminationControls, @ism:exemptFrom, @ism:FGIsourceOpen, @ism:FGIsourceProtected, @ism:nonICmarkings, @ism:nonUSControls, @ism:noticeType, @ism:ownerProducer, @ism:pocType, @ism:releasableTo, @ism:SARIdentifier, @ism:SCIcontrols) return if(count(distinct-values(tokenize(string($attr),' '))) != count(tokenize(string($attr),' '))                                         and not(local-name($attr)='derivedFrom' or local-name($attr)='classificationReason')) then $attr else null"/>
        <sch:let name="hasDups" value="count($dupAttrs)&gt;0"/>
        <sch:let name="dupValues" value="if ($hasDups) then  distinct-values(  for $attrib in $dupAttrs return     for $each in tokenize(string($attrib),' ') return     if(count(index-of(tokenize(string($attrib),' '), $each))&gt;1)     then concat(string($each),' in attribute ',$attrib/name(),'; ')     else null)     else null     "/>
        <sch:assert test="not($hasDups)" flag="error" role="error"> 
            [ISM-ID-00236][Error] Duplicate tokens are not permitted in ISM attributes. Duplicate values found: [<sch:value-of select="$dupValues"/>]</sch:assert>
    </sch:rule>
</sch:pattern>